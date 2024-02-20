use crate::valorant;
use ffmpeg_sidecar::command::FfmpegCommand;
use ffmpeg_sidecar::event::{FfmpegEvent, OutputVideoFrame};
use image::{ImageBuffer, Rgb};
use kdam::tqdm;
use lazy_static::lazy_static;
use log::{debug, warn};
use ndarray::{Array, ArrayBase, CowArray, CowRepr};
use ort::environment::Environment;
use ort::{GraphOptimizationLevel, LoggingLevel, OrtResult, Session, SessionBuilder, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;
use uuid::Uuid;
use valorant_api_official::response_types::matchdetails_v1::MatchDetailsV1;

const VIDEO_MATCH_SPLIT_THRESHOLD: u64 = 2 * 60 * 1000;

const VIDEO_ANALYSIS_RATE: usize = 6;

lazy_static! {
    static ref ORT_ENVIRONMENT: Arc<Environment> = Environment::builder()
        .with_name("clip-cutter")
        .with_log_level(LoggingLevel::Error)
        .build()
        .expect("Could not create environment")
        .into_arc();
    static ref ORT_SESSION: Session = SessionBuilder::new(&ORT_ENVIRONMENT)
        .expect("Could not create session builder")
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .expect("Could not set optimization level")
        .with_model_from_file("model.onnx")
        .expect("Could not load model");
}

pub(crate) fn get_match_interval(
    video_start: OffsetDateTime,
    valo_match: &MatchDetailsV1,
) -> (Duration, Duration) {
    let start = get_video_offset(video_start, valo_match);
    let end = start
        + Duration::from_millis(
            valorant::get_match_length(valo_match) + VIDEO_MATCH_SPLIT_THRESHOLD,
        );
    (start, end)
}

pub(crate) struct Metadata {
    pub(crate) track: String,
    pub(crate) title: String,
    pub(crate) episode_id: String,
    pub(crate) album: String,
    pub(crate) description: String,
    pub(crate) genre: String,
}
impl IntoIterator for Metadata {
    type Item = (String, String);
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> std::vec::IntoIter<Self::Item> {
        vec![
            ("track".to_string(), self.track),
            ("title".to_string(), self.title),
            ("episode_id".to_string(), self.episode_id),
            ("album".to_string(), self.album),
            ("description".to_string(), self.description),
            ("genre".to_string(), self.genre),
        ]
        .into_iter()
    }
}

pub(crate) fn split_video(
    path: &Path,
    out_path: &Path,
    start: &Duration,
    end: &Duration,
    copy_encoding: bool,
    metadata: Option<Metadata>,
) -> std::io::Result<PathBuf> {
    let encode = if copy_encoding { "copy" } else { "libx264" };
    let metadata = match metadata {
        Some(metadata) => metadata
            .into_iter()
            .map(|(k, v)| format!(" -metadata {}={}", k, v))
            .collect::<Vec<_>>()
            .join(""),
        None => "".to_string(),
    };
    let ffmpeg_split_command = format!(
        "ffmpeg -y -i {} -ss {} -to {} -c:a copy -c:v {} -copyts {} {}",
        path.to_str().unwrap(),
        format_ffmpeg_time(start, true),
        format_ffmpeg_time(end, true),
        encode,
        metadata,
        out_path.to_str().unwrap(),
    );
    std::process::Command::new("sh")
        .arg("-c")
        .arg(ffmpeg_split_command)
        .output()?;
    Ok(Path::new(&out_path).to_path_buf())
}

fn get_video_offset(video_start: OffsetDateTime, valo_match: &MatchDetailsV1) -> Duration {
    let match_start = valo_match.match_info.game_start_millis;
    let video_start = video_start.unix_timestamp() as u64 * 1000;
    Duration::from_millis(match_start - video_start)
}

pub(crate) fn format_ffmpeg_time(time: &Duration, with_millis: bool) -> String {
    let millis = time.as_millis();
    let hours = millis / 3600 / 1000;
    let minutes = millis / 60 / 1000 % 60;
    let seconds = millis / 1000 % 60;
    let millis = millis % 1000;
    if with_millis {
        format!("{:02}:{:02}:{:02}.{:03}", hours, minutes, seconds, millis)
    } else {
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    }
}

pub(crate) fn detect_kill_events(path: &Path) -> Vec<Duration> {
    let mut process = FfmpegCommand::new()
        .hwaccel("auto")
        .input(path.to_str().unwrap())
        .rate(VIDEO_ANALYSIS_RATE as f32)
        .filter("crop=200:200:in_w/2-100:0.7*in_h,scale=50:50")
        .no_audio()
        .rawvideo()
        .spawn()
        .unwrap();
    let video = process.iter().unwrap();

    let mut first_stamp = None;
    let mut wait_for_toggle = false;
    let mut kills = vec![];
    let mut num_consecutive_detections = 0;
    let mut num_consecutive_non_detections = 0;

    for (i, frame) in tqdm!(video.enumerate(), desc = "Detecting kill events") {
        if let FfmpegEvent::OutputFrame(frame) = frame {
            let has_kill = match has_kill(&frame) {
                Ok(v) => v,
                Err(e) => {
                    warn!("Failed to classify frame: {}", e);
                    continue;
                }
            };
            if cfg!(debug_assertions) {
                let image: ImageBuffer<Rgb<u8>, _> =
                    ImageBuffer::from_raw(frame.width, frame.height, frame.data.clone())
                        .expect("Failed to create image buffer");
                let label = if has_kill { "kill" } else { "no_kill" };
                image
                    .save(format!(
                        "kill-data/unclassified/{}/{}-{}.png",
                        label,
                        i,
                        Uuid::new_v4(),
                    ))
                    .unwrap();
            }

            if wait_for_toggle {
                if !has_kill {
                    num_consecutive_non_detections += 1;
                    if num_consecutive_non_detections > 2 * VIDEO_ANALYSIS_RATE {
                        wait_for_toggle = false;
                        num_consecutive_non_detections = 0;
                    }
                } else {
                    num_consecutive_non_detections = 0;
                }
                continue;
            }

            if !has_kill {
                if num_consecutive_detections > 0 {
                    warn!("Found kill event but no longer detecting");
                }
                num_consecutive_detections = 0;
                continue;
            }

            first_stamp = first_stamp.or(Some(frame.timestamp));
            num_consecutive_detections += 1;

            if num_consecutive_detections > 0 {
                debug!("Found kill event");
                kills.push(Duration::from_secs_f32(first_stamp.unwrap()));
                first_stamp = None;
                wait_for_toggle = true;
                num_consecutive_detections = 0;
            }
        }
    }
    kills
}

fn has_kill(frame: &OutputVideoFrame) -> OrtResult<bool> {
    let array = Array::from_shape_vec(
        (1, 3 * 50 * 50),
        frame
            .data
            .iter()
            .map(|v| *v as f32 / 255.)
            .collect::<Vec<_>>(),
    )
    .expect("Failed to create array");
    let array: ArrayBase<CowRepr<'_, f32>, _> = CowArray::from(array).into_dyn();
    let tensor = Value::from_array(ORT_SESSION.allocator(), &array)?;
    let outputs = ORT_SESSION.run(vec![tensor])?;
    let output = outputs.first().unwrap().try_extract()?;
    Ok(output.view().iter().all(|v: &i64| *v > 0))
}
