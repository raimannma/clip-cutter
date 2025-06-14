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
    static ref CLIP_DATE_PREFIX: Vec<format_description::FormatItem<'static>> =
        format_description::parse("[day]-[month]-[year]").unwrap();
    static ref CLIP_PADDING: (Duration, Duration) =
        (Duration::from_secs(10), Duration::from_secs(10));
}

#[derive(Parser, Debug, Clone, Eq, Hash, PartialEq)]
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
    #[arg(long)]
    exclude_category: Option<Vec<String>>,
    #[arg(long, default_value = "false")]
    only_customs: bool,
    #[arg(long, default_value = "0")]
    matches_after: u64,
    #[arg(long, default_value = "18446744073709551615")]
    matches_before: u64,
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

    let vod_ids = args
        .vod_ids
        .iter()
        .flat_map(|x| x.split(','))
        .flat_map(|x| x.split(' '))
        .flat_map(|x| x.split('\n'))
        .collect_vec();

    for vod_id in vod_ids {
        process_vod(vod_id.parse().unwrap(), &puuids, args.clone()).await;
    }
}

async fn process_vod(vod_id: usize, puuids: &HashSet<String>, args: Cli) {
    let vod_interval = twitch::get_vod_start_end(vod_id).await;
    let matches =
        valorant::find_valorant_matches_by_players(puuids, vod_interval, vod_id, args.force)
            .await
            .expect("Failed to find matches");

    for valo_match in matches {
        if args.only_customs && valo_match.match_info.provisioning_flow_id != "CustomGame" {
            debug!(
                "Skipping match: {:?} not a custom game",
                valo_match.match_info.match_id
            );
            continue;
        }
        if args.only_customs && valo_match.players.len() < 10 {
            debug!(
                "Skipping match: {:?} not enough players",
                valo_match.match_info.match_id
            );
            continue;
        }
        if valo_match.match_info.game_start_millis < args.matches_after {
            debug!(
                "Skipping match: {:?} before matches_after",
                valo_match.match_info.match_id
            );
            continue;
        }
        if valo_match.match_info.game_start_millis > args.matches_before {
            debug!(
                "Skipping match: {:?} before matches_before",
                valo_match.match_info.match_id
            );
            continue;
        }
        let match_id = valo_match.match_info.match_id;

        let processed_path = Path::new("/processed").join(format!("{vod_id}-{match_id}"));
        let failed_path = Path::new("/failed").join(format!("{vod_id}-{match_id}"));
        if process_match(
            puuids,
            vod_id,
            vod_interval,
            &valo_match,
            args.remove_matches,
            &args.category,
            &args.exclude_category,
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
    exclude_category: &Option<Vec<String>>,
) -> Option<()> {
    debug!(
        "Filtering for category: {:?} excluding category: {:?}",
        category, exclude_category
    );
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
        if let Some(exclude_category) = exclude_category.as_ref() {
            if exclude_category.contains(&event.category(puuids).await) {
                continue;
            }
        }
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
        Path::new("matches").join(format!("{}-{}.mkv", vod_id, valo_match.match_info.match_id));
    let (start, end) = match video::get_match_interval(vod_interval.0, valo_match) {
        Ok(interval) => interval,
        Err(msg) => {
            error!("Failed to get match interval: {}", msg);
            return None;
        }
    };
    valorant::save_match_video(&match_video_path, vod_id, start, end)
        .expect("Failed to save video");

    let min_offset = valo_match.match_info.queue_id.map_or(40000, |q| match q {
        Queue::DEATHMATCH => 0,
        Queue::COMPETITIVE => 60000,
        _ => 40000,
    });

    let kill_timestamps = video::detect_kill_timestamps(&match_video_path, min_offset);
    let detected_kill_events = video::detect_kill_events(min_offset, 0, &kill_timestamps)
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

    let offset = match offset::get_offset(&detected_kill_events, &match_kill_events, min_offset) {
        Some(offset) => offset,
        None => {
            let detected_kill_events = video::detect_kill_events(min_offset, 1, &kill_timestamps)
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
            offset::get_offset(&detected_kill_events, &match_kill_events, min_offset)?
        }
    };

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

        // timestamp of event in vod
        let event_vod_time = match_date - vod_interval.0 + start;
        let seconds = event_vod_time.whole_seconds();
        let hours = seconds / 3600;
        let minutes = (seconds % 3600) / 60;
        let seconds = seconds % 60;
        let event_vod_time = format!("{hours:02}-{minutes:02}-{seconds:02}");

        let clip_name = format!("{event_vod_time}_{event_date}_{map_name}_{name_postfix}.mp4");
        let clip_path = Path::new("clips")
            .join(format!(
                "{}_{}",
                vod_id,
                vod_interval.0.format(&CLIP_DATE_PREFIX).unwrap()
            ))
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
            start,
            end,
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
