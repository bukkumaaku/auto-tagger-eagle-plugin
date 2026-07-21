use protocol::{
    VideoCleanupFramesRequest, VideoCleanupFramesResult, VideoExtractFramesRequest,
    VideoExtractFramesResult,
};
use video_tagger::{cleanup_extracted_frames, extract_video_frames};

use super::{HandlerError, HandlerResult};

pub fn extract(request: VideoExtractFramesRequest) -> HandlerResult<VideoExtractFramesResult> {
    let frames = extract_video_frames(
        &request.video_path,
        &request.ffmpeg_path,
        &request.ffprobe_path,
    )
    .map_err(|error| HandlerError::new("VIDEO_EXTRACT_FAILED", error.to_string()))?;
    Ok(VideoExtractFramesResult {
        video_path: frames.video_path,
        duration_seconds: frames.duration_seconds,
        frame_paths: frames.frame_paths,
        directory: frames.directory,
    })
}

pub fn cleanup(request: VideoCleanupFramesRequest) -> HandlerResult<VideoCleanupFramesResult> {
    let removed = cleanup_extracted_frames(&request.directory)
        .map_err(|error| HandlerError::new("VIDEO_CLEANUP_FAILED", error.to_string()))?;
    Ok(VideoCleanupFramesResult { removed })
}
