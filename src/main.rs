mod events;
mod offset;
mod twitch;
mod valorant;
mod video;

use crate::events::Event;
use crate::video::Metadata;
use clap::Parser;
use dotenv::dotenv;
use events::MatchEvent;
use filetime_creation::{set_file_times, FileTime};
use itertools::Itertools;
use kdam::tqdm;
use lazy_static::lazy_static;
use log::{debug, error, info};
use std::collections::HashSet;
use std::fmt::Debug;
use std::path::Path;
use std::time::{Duration, SystemTime};
use time::{format_description, OffsetDateTime};
use valorant_api_official::enums::queue::Queue;
use valorant_api_official::response_types::matchdetails_v1::MatchDetailsV1;

lazy_static! {
    static ref CLIP_DATE_TIME_PREFIX: Vec<format_description::FormatItem<'static>> =
        format_description::parse("[day]-[month]-[year]_[hour]-[minute]-[second]").unwrap();
    static ref CLIP_PADDING: (Duration, Duration) =
        (Duration::from_secs(10), Duration::from_secs(10));
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(next_line_help = true)]
struct Cli {
    #[arg(short, long, required = true)]
    vod_ids: Vec<String>,
    #[arg(short, long, required = true)]
    riot_ids: Vec<String>,
    #[arg(long, default_value = "false")]
    remove_matches: bool,
    #[arg(long, default_value = "false")]
    force: bool,
    #[arg(long)]
    category: Option<Vec<String>>,
}

#[tokio::main]
pub async fn main() {
    dotenv().ok();
    env_logger::init();
    ffmpeg_sidecar::download::auto_download().expect("Failed to download ffmpeg");
    let args = Cli::parse();

    let puuids = futures::future::join_all(
        args.riot_ids
            .iter()
            .flat_map(|riot_id| riot_id.split(','))
            .map(valorant::get_puuid),
    )
    .await
    .into_iter()
    .filter_map(|x| x.ok())
    .collect::<HashSet<_>>();

    for vod_id in args.vod_ids {
        process_vod(
            vod_id.parse().unwrap(),
            &puuids,
            args.remove_matches,
            args.force,
            &args.category,
        )
        .await;
    }
}

async fn process_vod(
    vod_id: usize,
    puuids: &HashSet<String>,
    remove_matches: bool,
    force: bool,
    category: &Option<Vec<String>>,
) {
    let vod_interval = twitch::get_vod_start_end(vod_id).await;
    let matches = valorant::find_valorant_matches_by_players(puuids, vod_interval)
        .await
        .expect("Failed to find matches");

    for valo_match in matches {
        let match_id = valo_match.match_info.match_id;

        let processed_path = Path::new("/processed").join(format!("{}-{}", vod_id, match_id));
        let failed_path = Path::new("/failed").join(format!("{}-{}", vod_id, match_id));
        if !force && (processed_path.exists() || failed_path.exists()) {
            debug!("Skipping match: {:?}", match_id);
            continue;
        }

        if process_match(
            puuids,
            vod_id,
            vod_interval,
            &valo_match,
            remove_matches,
            category,
        )
        .await
        .is_some()
        {
            std::fs::create_dir_all(processed_path.parent().unwrap()).ok();
            std::fs::write(processed_path, "").unwrap();
        } else {
            std::fs::create_dir_all(failed_path.parent().unwrap()).ok();
            std::fs::write(failed_path, "").unwrap();
        }
    }
}

async fn process_match(
    puuids: &HashSet<String>,
    vod_id: usize,
    vod_interval: (OffsetDateTime, OffsetDateTime),
    valo_match: &MatchDetailsV1,
    remove_matches: bool,
    category: &Option<Vec<String>>,
) -> Option<()> {
    debug!("Filtering for category: {:?}", category);
    let events = events::build_events(valo_match)
        .into_iter()
        .filter(|e| match e {
            Event::Kill(e) => e.is_from_puuids(puuids) || e.is_against_puuids(puuids),
            Event::MultiKill(e) => e.is_from_puuids(puuids),
            Event::Clutch(e) => e.is_from_puuids(puuids),
            Event::DoubleKill(e) => e.is_from_puuids(puuids),
            Event::Plant(e) => e.is_from_puuids(puuids),
            Event::Defuse(e) => e.is_from_puuids(puuids),
            Event::Ace(e) => e.is_from_puuids(puuids),
            Event::Retake(e) => e.is_from_puuids(puuids),
        })
        .collect_vec();

    let mut filtered_events = vec![];
    for event in events {
        if let Some(category) = category.as_ref() {
            if category.contains(&event.category(puuids).await) {
                filtered_events.push(event);
            }
        } else {
            filtered_events.push(event);
        }
    }
    let events = filtered_events;

    info!("Found {} events", events.len());

    if events.is_empty() {
        return None;
    }

    let match_kill_events = valorant::get_match_kills(valo_match)
        .iter()
        .filter(|k| puuids.contains(&k.killer))
        .map(|v| v.time_since_game_start_millis)
        .map(Duration::from_millis)
        .sorted()
        .collect::<Vec<_>>();

    if match_kill_events.is_empty() {
        error!("No match kill events found");
        return None;
    }

    let match_video_path =
        Path::new("matches").join(format!("{}.mp4", valo_match.match_info.match_id));
    let (start, end) = video::get_match_interval(vod_interval.0, valo_match);
    valorant::save_match_video(&match_video_path, vod_id, start, end)
        .expect("Failed to save video");

    let min_offset = match valo_match.match_info.queue_id.unwrap_or(Queue::COMPETITIVE) {
        Queue::DEATHMATCH => 0,
        Queue::COMPETITIVE => 60000,
        _ => 40000,
    };

    let detected_kill_events = video::detect_kill_events(&match_video_path, min_offset)
        .into_iter()
        .sorted()
        .collect::<Vec<_>>();

    if detected_kill_events.is_empty() {
        error!("No detected kill events found");
        return None;
    }

    if (detected_kill_events.len() as i64) < (match_kill_events.len() as i64 / 2) {
        error!("Fewer detected kill events than match kill events: Detected Kills: {}, Match Kills: {}", detected_kill_events.len(), match_kill_events.len());
        return None;
    }

    let offset = offset::get_offset(&detected_kill_events, &match_kill_events, min_offset)?;

    let offset = Duration::from_millis(offset - 350);

    let match_date =
        OffsetDateTime::from_unix_timestamp(valo_match.match_info.game_start_millis as i64 / 1000)
            .ok()?;
    let map_name = valorant::get_map_name(&valo_match.match_info.map_id)
        .await
        .expect("Failed to get map name")
        .expect("Failed to get map name");
    let game_mode = &valo_match
        .match_info
        .queue_id
        .map_or("other".to_string(), |q| q.to_string());

    for event in tqdm!(events.iter(), desc = "Saving clips", total = events.len()) {
        let category = event.category(puuids).await;
        let name_postfix = event
            .name_postfix(valo_match)
            .await
            .replace([' ', '/'], "_");

        let (start, end) = event.game_time_interval();
        let event_date = (match_date + start).format(&CLIP_DATE_TIME_PREFIX).unwrap();
        let (start, end) = (start + offset, end + offset);

        let clip_name = format!("{}_{}_{}.mp4", event_date, map_name, name_postfix);
        let clip_path = Path::new("clips")
            .join(game_mode)
            .join(&category)
            .join(clip_name);
        std::fs::create_dir_all(clip_path.parent()?).ok()?;

        let (start, end) = (start - CLIP_PADDING.0, end + CLIP_PADDING.1);
        let metadata = Metadata {
            track: offset.as_millis().to_string(),
            title: category.to_string(),
            album: valo_match.match_info.match_id.to_string(),
            episode_id: valo_match.match_info.season_id.to_string(),
            description: serde_json::to_string(&event).unwrap(),
            genre: game_mode.to_string(),
        };
        if let Err(e) = video::split_video(
            &match_video_path,
            &clip_path,
            &start,
            &end,
            true,
            Some(metadata),
        ) {
            error!("Failed to save clip: {}", e)
        } else {
            let file_time = FileTime::from_system_time(SystemTime::from(match_date + start));
            set_file_times(&clip_path, file_time, file_time, file_time).ok()?;
        }
    }

    if remove_matches {
        std::fs::remove_file(match_video_path).ok();
    }
    Some(())
}
