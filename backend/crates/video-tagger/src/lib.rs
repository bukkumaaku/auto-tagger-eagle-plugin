use std::{
    cmp::Ordering,
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering},
    time::{SystemTime, UNIX_EPOCH},
};

use thiserror::Error;
use wd_tagger::{TagScore, TaggerError, WdTagger};

static VIDEO_IN_PROGRESS: AtomicBool = AtomicBool::new(false);
static NEXT_TEMP_DIRECTORY: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug)]
pub struct TagVideoRequest {
    pub video_path: String,
    pub ffmpeg_path: String,
    pub ffprobe_path: String,
    pub frame_count: usize,
    pub batch_size: usize,
    pub threshold: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FramePrediction {
    pub frame_number: usize,
    pub timestamp_seconds: f64,
    pub tags: Vec<TagScore>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VideoPrediction {
    pub video_path: String,
    pub duration_seconds: f64,
    pub frames: Vec<FramePrediction>,
    pub tags: Vec<TagScore>,
}

#[derive(Debug, Error)]
pub enum VideoTaggerError {
    #[error("another video is already being tagged")]
    Busy,

    #[error("video file does not exist: {0}")]
    VideoNotFound(String),

    #[error("frameCount must be greater than zero")]
    InvalidFrameCount,

    #[error("batchSize must be greater than zero")]
    InvalidBatchSize,

    #[error("threshold must be between 0 and 1, got {0}")]
    InvalidThreshold(f32),

    #[error("failed to create temporary frame directory {path}: {source}")]
    CreateTempDirectory { path: PathBuf, source: io::Error },

    #[error("failed to run ffprobe at {path}: {source}")]
    StartFfprobe { path: String, source: io::Error },

    #[error("ffprobe failed with status {status}: {stderr}")]
    FfprobeFailed { status: String, stderr: String },

    #[error("ffprobe returned invalid video duration `{output}`")]
    InvalidDuration { output: String },

    #[error("failed to run ffmpeg at {path}: {source}")]
    StartFfmpeg { path: String, source: io::Error },

    #[error("ffmpeg failed at {timestamp_seconds:.3}s with status {status}: {stderr}")]
    FfmpegFailed {
        timestamp_seconds: f64,
        status: String,
        stderr: String,
    },

    #[error("WD tagging failed: {0}")]
    Tagger(#[from] TaggerError),
}

pub fn tag_video(
    tagger: &mut WdTagger,
    request: TagVideoRequest,
) -> Result<VideoPrediction, VideoTaggerError> {
    let _active = ActiveVideo::acquire()?;
    validate_request(&request)?;

    let duration_seconds = probe_duration(&request.ffprobe_path, &request.video_path)?;
    let timestamps = frame_timestamps(duration_seconds, request.frame_count);
    let temp_frames = TempFrames::create()?;
    let mut frame_paths = Vec::with_capacity(timestamps.len());

    for (index, timestamp_seconds) in timestamps.iter().copied().enumerate() {
        let frame_path = temp_frames.path().join(format!("frame-{index:04}.png"));
        extract_frame(
            &request.ffmpeg_path,
            &request.video_path,
            timestamp_seconds,
            &frame_path,
        )?;
        frame_paths.push(frame_path.to_string_lossy().into_owned());
    }

    let mut frames = Vec::with_capacity(frame_paths.len());
    for (batch_index, paths) in frame_paths.chunks(request.batch_size).enumerate() {
        let predictions = tagger.predict(paths, request.threshold)?;
        let frame_offset = batch_index * request.batch_size;

        for (index, prediction) in predictions.into_iter().enumerate() {
            let frame_index = frame_offset + index;
            frames.push(FramePrediction {
                frame_number: frame_index + 1,
                timestamp_seconds: timestamps[frame_index],
                tags: prediction.tags,
            });
        }
    }

    let tags = merge_tags(&frames);
    Ok(VideoPrediction {
        video_path: request.video_path,
        duration_seconds,
        frames,
        tags,
    })
}

fn validate_request(request: &TagVideoRequest) -> Result<(), VideoTaggerError> {
    if !Path::new(&request.video_path).is_file() {
        return Err(VideoTaggerError::VideoNotFound(request.video_path.clone()));
    }
    if request.frame_count == 0 {
        return Err(VideoTaggerError::InvalidFrameCount);
    }
    if request.batch_size == 0 {
        return Err(VideoTaggerError::InvalidBatchSize);
    }
    if !(0.0..=1.0).contains(&request.threshold) {
        return Err(VideoTaggerError::InvalidThreshold(request.threshold));
    }
    Ok(())
}

fn probe_duration(ffprobe_path: &str, video_path: &str) -> Result<f64, VideoTaggerError> {
    let output = Command::new(ffprobe_path)
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            video_path,
        ])
        .output()
        .map_err(|source| VideoTaggerError::StartFfprobe {
            path: ffprobe_path.to_string(),
            source,
        })?;

    if !output.status.success() {
        return Err(VideoTaggerError::FfprobeFailed {
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }

    let duration = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let parsed = duration
        .parse::<f64>()
        .map_err(|_| VideoTaggerError::InvalidDuration {
            output: duration.clone(),
        })?;
    if !parsed.is_finite() || parsed <= 0.0 {
        return Err(VideoTaggerError::InvalidDuration { output: duration });
    }
    Ok(parsed)
}

fn extract_frame(
    ffmpeg_path: &str,
    video_path: &str,
    timestamp_seconds: f64,
    destination: &Path,
) -> Result<(), VideoTaggerError> {
    let timestamp = format!("{timestamp_seconds:.6}");
    let output = Command::new(ffmpeg_path)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-ss",
            &timestamp,
            "-i",
        ])
        .arg(video_path)
        .args(["-frames:v", "1", "-y"])
        .arg(destination)
        .output()
        .map_err(|source| VideoTaggerError::StartFfmpeg {
            path: ffmpeg_path.to_string(),
            source,
        })?;

    if !output.status.success() {
        return Err(VideoTaggerError::FfmpegFailed {
            timestamp_seconds,
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(())
}

fn frame_timestamps(duration_seconds: f64, frame_count: usize) -> Vec<f64> {
    if frame_count == 1 {
        return vec![duration_seconds * 0.5];
    }
    if frame_count == 6 {
        return [1.0, 20.0, 40.0, 60.0, 80.0, 99.0]
            .into_iter()
            .map(|percentage| duration_seconds * percentage / 100.0)
            .collect();
    }

    (0..frame_count)
        .map(|index| {
            let percentage = 1.0 + (98.0 * index as f64 / (frame_count - 1) as f64);
            duration_seconds * percentage / 100.0
        })
        .collect()
}

fn merge_tags(frames: &[FramePrediction]) -> Vec<TagScore> {
    let mut scores = HashMap::<String, f32>::new();
    for tag in frames.iter().flat_map(|frame| &frame.tags) {
        scores
            .entry(tag.name.clone())
            .and_modify(|score| *score = score.max(tag.score))
            .or_insert(tag.score);
    }

    let mut tags = scores
        .into_iter()
        .map(|(name, score)| TagScore { name, score })
        .collect::<Vec<_>>();
    tags.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(Ordering::Equal)
    });
    tags
}

struct ActiveVideo;

impl ActiveVideo {
    fn acquire() -> Result<Self, VideoTaggerError> {
        VIDEO_IN_PROGRESS
            .compare_exchange(false, true, AtomicOrdering::AcqRel, AtomicOrdering::Acquire)
            .map(|_| Self)
            .map_err(|_| VideoTaggerError::Busy)
    }
}

impl Drop for ActiveVideo {
    fn drop(&mut self) {
        VIDEO_IN_PROGRESS.store(false, AtomicOrdering::Release);
    }
}

struct TempFrames {
    path: PathBuf,
}

impl TempFrames {
    fn create() -> Result<Self, VideoTaggerError> {
        let unique = NEXT_TEMP_DIRECTORY.fetch_add(1, AtomicOrdering::Relaxed);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir()
            .join("eagle-auto-tagger")
            .join(format!("video-{}-{timestamp}-{unique}", std::process::id()));
        fs::create_dir_all(&path).map_err(|source| VideoTaggerError::CreateTempDirectory {
            path: path.clone(),
            source,
        })?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempFrames {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_frames_follow_the_js_one_to_ninety_nine_percent_range() {
        let timestamps = frame_timestamps(100.0, 6);

        assert_eq!(timestamps, vec![1.0, 20.0, 40.0, 60.0, 80.0, 99.0]);
    }

    #[test]
    fn one_frame_uses_the_middle_of_the_video() {
        assert_eq!(frame_timestamps(20.0, 1), vec![10.0]);
    }

    #[test]
    fn merged_tags_keep_the_highest_score() {
        let frames = vec![
            FramePrediction {
                frame_number: 1,
                timestamp_seconds: 1.0,
                tags: vec![TagScore {
                    name: "city".to_string(),
                    score: 0.7,
                }],
            },
            FramePrediction {
                frame_number: 2,
                timestamp_seconds: 2.0,
                tags: vec![TagScore {
                    name: "city".to_string(),
                    score: 0.9,
                }],
            },
        ];

        assert_eq!(merge_tags(&frames)[0].score, 0.9);
    }
}
