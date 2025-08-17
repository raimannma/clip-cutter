use crate::video::format_ffmpeg_time;
use lazy_static::lazy_static;
use log::{debug, info};
use serde::Deserialize;
use std::path::Path;
use std::process::ExitStatus;
use std::time::Duration;
use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, PrimitiveDateTime, UtcOffset};

lazy_static! {
    static ref TWITCH_CLIENT_ID: String =
        std::env::var("TWITCH_CLIENT_ID").expect("Failed to get TWITCH_CLIENT_ID");
    static ref TWITCH_ACCESS_TOKEN: String =
        std::env::var("TWITCH_ACCESS_TOKEN").expect("Failed to get TWITCH_CLIENT_ID");
}

#[derive(Debug, Deserialize)]
struct ApiData<T> {
    data: T,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct TwitchVideo {
    id: String,
    created_at: String,
    duration: String,
}

pub async fn get_vod_start_end(vod_id: usize) -> (OffsetDateTime, OffsetDateTime) {
    let client = reqwest::Client::new();
    let response: ApiData<Vec<TwitchVideo>> = client
        .get(format!("https://api.twitch.tv/helix/videos?id={vod_id}"))
        .header("Client-ID", TWITCH_CLIENT_ID.as_str())
        .bearer_auth(TWITCH_ACCESS_TOKEN.as_str())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let twitch_video = response
        .data
        .into_iter()
        .find(|video| video.id == vod_id.to_string())
        .unwrap_or_else(|| panic!("Failed to find video: {vod_id}"));

    let vod_start = twitch_video.created_at.clone();
    let vod_length = parse_length(twitch_video.duration.as_str());
    let start_time = PrimitiveDateTime::parse(vod_start.as_str(), &Rfc3339)
        .expect("Failed to parse start time")
        .assume_offset(UtcOffset::UTC);
    let end_time = start_time + time::Duration::new(vod_length as i64, 0);
    (start_time, end_time)
}

pub(crate) fn download_vod(
    vod_id: usize,
    out_path: &Path,
    start: Duration,
    end: Duration,
) -> std::io::Result<ExitStatus> {
    if out_path.exists() {
        debug!("VOD already exists: {}", out_path.display());
        return Ok(ExitStatus::default());
    }
    info!("Saving VOD: {} from {:?} to {:?}", vod_id, start, end);
    std::fs::create_dir_all(out_path.parent().unwrap())?;
    // if let Err(e) = download_with_twitchdl(vod_id, out_path, start, end) {
    //     warn!("Twitch-DL failed: {}", e);
    // }
    if !out_path.exists() {
        download_with_ytdlp(vod_id, out_path, start, end)
    } else {
        Ok(ExitStatus::default())
    }
}

fn download_with_twitchdl(
    vod_id: usize,
    out_path: &Path,
    start: Duration,
    end: Duration,
) -> std::io::Result<ExitStatus> {
    std::process::Command::new("twitch-dl")
        .arg("download")
        .arg(vod_id.to_string())
        .arg("-q")
        .arg("source")
        .arg("-s")
        .arg(format_ffmpeg_time(start, false))
        .arg("-e")
        .arg(format_ffmpeg_time(end, false))
        .arg("-o")
        .arg(out_path)
        .status()
}

fn download_with_ytdlp(
    vod_id: usize,
    out_path: &Path,
    start: Duration,
    end: Duration,
) -> std::io::Result<ExitStatus> {
    let mut cmd = std::process::Command::new("yt-dlp");
    cmd.arg("--get-url")
        .arg("-f")
        .arg("b")
        .arg(format!("https://www.twitch.tv/videos/{vod_id}"));
    debug!("Running command: {:?}", cmd);
    let download_link = cmd.output()?.stdout;
    let download_link = String::from_utf8(download_link).unwrap();
    let download_link = download_link.trim();
    let mut cmd = std::process::Command::new("ffmpeg");
    cmd.arg("-y")
        .arg("-ss")
        .arg(format_ffmpeg_time(start, true))
        .arg("-to")
        .arg(format_ffmpeg_time(end, true))
        .arg("-i")
        .arg(download_link)
        .arg("-y")
        .arg("-c")
        .arg("copy")
        .arg(out_path);
    debug!("Running command: {:?}", cmd);
    cmd.status()
}

pub fn parse_length(length: &str) -> usize {
    let parts: Vec<&str> = length.split_inclusive(['h', 'm', 's']).collect();
    let mut seconds: usize = 0;
    for part in parts {
        if part.ends_with('h') {
            seconds += part.strip_suffix('h').unwrap().parse::<usize>().unwrap() * 60 * 60;
        } else if part.ends_with('m') {
            seconds += part.strip_suffix('m').unwrap().parse::<usize>().unwrap() * 60;
        } else if part.ends_with('s') {
            seconds += part.strip_suffix('s').unwrap().parse::<usize>().unwrap();
        }
    }
    seconds
}
