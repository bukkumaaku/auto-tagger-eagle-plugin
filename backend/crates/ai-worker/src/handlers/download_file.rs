use std::path::PathBuf;

use downloader::{DownloadEvent, DownloadRequest, Downloader};
use protocol::{DownloadFileProgress, DownloadFileRequest, DownloadFileResult, ProgressPayload};

use super::{EventEmitter, HandlerError, HandlerResult};

pub fn handle(
    request: DownloadFileRequest,
    events: &mut dyn EventEmitter,
) -> HandlerResult<DownloadFileResult> {
    let downloader = Downloader::new()
        .map_err(|error| HandlerError::new("DOWNLOAD_CLIENT_FAILED", error.to_string()))?;
    let destination = PathBuf::from(&request.destination);
    let mut event_error = None;

    let report = downloader
        .download_blocking(DownloadRequest::new(request.url, &destination), |event| {
            if event_error.is_some() {
                return;
            }

            if let DownloadEvent::Progress(progress) = event {
                event_error = events
                    .progress(ProgressPayload::DownloadFile(DownloadFileProgress {
                        downloaded_bytes: progress.downloaded_bytes,
                        remaining_bytes: progress.remaining_bytes,
                        total_bytes: progress.total_bytes,
                        bytes_per_second: progress.bytes_per_second,
                        percentage: progress.percentage,
                        elapsed_milliseconds: duration_milliseconds(progress.elapsed),
                    }))
                    .err();
            }
        })
        .map_err(|error| HandlerError::new("DOWNLOAD_FAILED", error.to_string()))?;

    if let Some(error) = event_error {
        return Err(error);
    }

    Ok(DownloadFileResult {
        requested_url: report.requested_url,
        final_url: report.final_url,
        destination: report.destination.to_string_lossy().into_owned(),
        downloaded_bytes: report.downloaded_bytes,
        total_bytes: report.total_bytes,
        average_bytes_per_second: report.average_bytes_per_second,
        elapsed_milliseconds: duration_milliseconds(report.elapsed),
    })
}

fn duration_milliseconds(duration: std::time::Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}
