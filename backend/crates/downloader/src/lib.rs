use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use futures_util::StreamExt;
use reqwest::{Client, StatusCode, Url, redirect::Policy};
use thiserror::Error;
use tokio::{
    fs,
    io::AsyncWriteExt,
    time::{MissedTickBehavior, interval},
};

const DEFAULT_PROGRESS_INTERVAL: Duration = Duration::from_millis(500);
const DEFAULT_REDIRECT_LIMIT: usize = 10;

#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("progress interval must be greater than zero")]
    InvalidProgressInterval,

    #[error("failed to create download runtime: {0}")]
    Runtime(#[source] std::io::Error),

    #[error("failed to build HTTP client: {0}")]
    Client(#[source] reqwest::Error),

    #[error("download request failed: {0}")]
    Request(#[source] reqwest::Error),

    #[error("download returned HTTP status {status} for {url}")]
    HttpStatus { status: StatusCode, url: Url },

    #[error("download file I/O failed at `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error(
        "failed to replace `{destination}`: {replace_error}; restoring backup `{backup}` also failed: {restore_error}"
    )]
    RestoreFailed {
        destination: PathBuf,
        backup: PathBuf,
        replace_error: std::io::Error,
        restore_error: std::io::Error,
    },

    #[error("download size mismatch: expected {expected} bytes, received {actual} bytes")]
    SizeMismatch { expected: u64, actual: u64 },
}

#[derive(Debug, Clone)]
pub struct DownloadRequest {
    pub url: String,
    pub destination: PathBuf,
}

impl DownloadRequest {
    pub fn new(url: impl Into<String>, destination: impl Into<PathBuf>) -> Self {
        Self {
            url: url.into(),
            destination: destination.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub downloaded_bytes: u64,
    pub remaining_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub bytes_per_second: f64,
    pub percentage: Option<f64>,
    pub elapsed: Duration,
}

#[derive(Debug, Clone)]
pub struct DownloadReport {
    pub requested_url: String,
    pub final_url: String,
    pub destination: PathBuf,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub average_bytes_per_second: f64,
    pub elapsed: Duration,
}

#[derive(Debug, Clone)]
pub enum DownloadEvent {
    Started {
        requested_url: String,
        final_url: String,
        total_bytes: Option<u64>,
    },
    Progress(DownloadProgress),
    Finished(DownloadReport),
}

#[derive(Debug, Clone)]
pub struct DownloaderBuilder {
    progress_interval: Duration,
    redirect_limit: usize,
}

impl Default for DownloaderBuilder {
    fn default() -> Self {
        Self {
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            redirect_limit: DEFAULT_REDIRECT_LIMIT,
        }
    }
}

impl DownloaderBuilder {
    pub fn progress_interval(mut self, progress_interval: Duration) -> Self {
        self.progress_interval = progress_interval;
        self
    }

    pub fn redirect_limit(mut self, redirect_limit: usize) -> Self {
        self.redirect_limit = redirect_limit;
        self
    }

    pub fn build(self) -> Result<Downloader, DownloadError> {
        if self.progress_interval.is_zero() {
            return Err(DownloadError::InvalidProgressInterval);
        }

        let client = Client::builder()
            .redirect(Policy::limited(self.redirect_limit))
            .build()
            .map_err(DownloadError::Client)?;

        Ok(Downloader {
            client,
            progress_interval: self.progress_interval,
        })
    }
}

#[derive(Clone)]
pub struct Downloader {
    client: Client,
    progress_interval: Duration,
}

impl Downloader {
    pub fn builder() -> DownloaderBuilder {
        DownloaderBuilder::default()
    }

    pub fn new() -> Result<Self, DownloadError> {
        Self::builder().build()
    }

    pub fn download_blocking<F>(
        &self,
        request: DownloadRequest,
        on_event: F,
    ) -> Result<DownloadReport, DownloadError>
    where
        F: FnMut(DownloadEvent),
    {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(DownloadError::Runtime)?;

        runtime.block_on(self.download(request, on_event))
    }

    pub async fn download<F>(
        &self,
        request: DownloadRequest,
        mut on_event: F,
    ) -> Result<DownloadReport, DownloadError>
    where
        F: FnMut(DownloadEvent),
    {
        let requested_url = request.url.clone();
        let response = self
            .client
            .get(&request.url)
            .send()
            .await
            .map_err(DownloadError::Request)?;

        let status = response.status();
        let final_url = response.url().clone();
        if !status.is_success() {
            return Err(DownloadError::HttpStatus {
                status,
                url: final_url,
            });
        }

        let total_bytes = response.content_length();
        on_event(DownloadEvent::Started {
            requested_url: requested_url.clone(),
            final_url: final_url.to_string(),
            total_bytes,
        });

        ensure_parent_directory(&request.destination).await?;
        let partial_path = partial_path(&request.destination);
        let result = self
            .download_response(
                response,
                &requested_url,
                &final_url,
                &request.destination,
                &partial_path,
                total_bytes,
                &mut on_event,
            )
            .await;

        if result.is_err() {
            let _ = fs::remove_file(&partial_path).await;
        }

        result
    }

    #[allow(clippy::too_many_arguments)]
    async fn download_response<F>(
        &self,
        response: reqwest::Response,
        requested_url: &str,
        final_url: &Url,
        destination: &Path,
        partial_path: &Path,
        total_bytes: Option<u64>,
        on_event: &mut F,
    ) -> Result<DownloadReport, DownloadError>
    where
        F: FnMut(DownloadEvent),
    {
        let mut file = fs::File::create(partial_path)
            .await
            .map_err(|source| io_error(partial_path, source))?;
        let mut stream = response.bytes_stream();
        let started_at = Instant::now();
        let mut last_report_at = started_at;
        let mut downloaded_at_last_report = 0_u64;
        let mut downloaded_bytes = 0_u64;
        let mut progress_timer = interval(self.progress_interval);
        progress_timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
        progress_timer.tick().await;

        loop {
            tokio::select! {
                chunk = stream.next() => {
                    match chunk {
                        Some(Ok(chunk)) => {
                            file.write_all(&chunk)
                                .await
                                .map_err(|source| io_error(partial_path, source))?;
                            downloaded_bytes += chunk.len() as u64;
                        }
                        Some(Err(error)) => return Err(DownloadError::Request(error)),
                        None => break,
                    }
                }
                _ = progress_timer.tick() => {
                    let now = Instant::now();
                    let current_speed = window_speed(
                        downloaded_bytes,
                        downloaded_at_last_report,
                        now.duration_since(last_report_at),
                    );
                    emit_progress(
                        on_event,
                        downloaded_bytes,
                        total_bytes,
                        current_speed,
                        started_at.elapsed(),
                    );
                    downloaded_at_last_report = downloaded_bytes;
                    last_report_at = now;
                }
            }
        }

        file.flush()
            .await
            .map_err(|source| io_error(partial_path, source))?;
        file.sync_all()
            .await
            .map_err(|source| io_error(partial_path, source))?;
        drop(file);

        if let Some(expected) = total_bytes
            && expected != downloaded_bytes
        {
            return Err(DownloadError::SizeMismatch {
                expected,
                actual: downloaded_bytes,
            });
        }

        replace_destination(partial_path, destination).await?;

        let elapsed = started_at.elapsed();
        let completed_total = total_bytes.or(Some(downloaded_bytes));
        let current_speed = window_speed(
            downloaded_bytes,
            downloaded_at_last_report,
            Instant::now().duration_since(last_report_at),
        );
        emit_progress(
            on_event,
            downloaded_bytes,
            completed_total,
            current_speed,
            elapsed,
        );

        let report = DownloadReport {
            requested_url: requested_url.to_string(),
            final_url: final_url.to_string(),
            destination: destination.to_path_buf(),
            downloaded_bytes,
            total_bytes: completed_total,
            average_bytes_per_second: average_speed(downloaded_bytes, elapsed),
            elapsed,
        };
        on_event(DownloadEvent::Finished(report.clone()));

        Ok(report)
    }
}

fn emit_progress<F>(
    on_event: &mut F,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    bytes_per_second: f64,
    elapsed: Duration,
) where
    F: FnMut(DownloadEvent),
{
    let remaining_bytes = total_bytes.map(|total| total.saturating_sub(downloaded_bytes));
    let percentage = total_bytes
        .filter(|total| *total > 0)
        .map(|total| ((downloaded_bytes as f64 / total as f64) * 100.0).min(100.0));

    on_event(DownloadEvent::Progress(DownloadProgress {
        downloaded_bytes,
        remaining_bytes,
        total_bytes,
        bytes_per_second,
        percentage,
        elapsed,
    }));
}

fn window_speed(current_bytes: u64, previous_bytes: u64, elapsed: Duration) -> f64 {
    if elapsed.is_zero() {
        return 0.0;
    }

    current_bytes.saturating_sub(previous_bytes) as f64 / elapsed.as_secs_f64()
}

fn average_speed(downloaded_bytes: u64, elapsed: Duration) -> f64 {
    if elapsed.is_zero() {
        return 0.0;
    }

    downloaded_bytes as f64 / elapsed.as_secs_f64()
}

async fn ensure_parent_directory(destination: &Path) -> Result<(), DownloadError> {
    if let Some(parent) = destination.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .await
            .map_err(|source| io_error(parent, source))?;
    }

    Ok(())
}

async fn replace_destination(partial: &Path, destination: &Path) -> Result<(), DownloadError> {
    let backup = backup_path(destination);
    let destination_exists = file_exists(destination).await?;
    if file_exists(&backup).await? {
        if destination_exists {
            fs::remove_file(&backup)
                .await
                .map_err(|source| io_error(&backup, source))?;
        } else {
            fs::rename(&backup, destination)
                .await
                .map_err(|source| io_error(destination, source))?;
        }
    }

    let had_destination = match fs::rename(destination, &backup).await {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(source) => return Err(io_error(destination, source)),
    };

    if let Err(source) = fs::rename(partial, destination).await {
        if had_destination && let Err(restore_error) = fs::rename(&backup, destination).await {
            return Err(DownloadError::RestoreFailed {
                destination: destination.to_path_buf(),
                backup,
                replace_error: source,
                restore_error,
            });
        }
        return Err(io_error(destination, source));
    }

    if had_destination {
        let _ = fs::remove_file(&backup).await;
    }
    Ok(())
}

async fn file_exists(path: &Path) -> Result<bool, DownloadError> {
    match fs::metadata(path).await {
        Ok(metadata) => Ok(metadata.is_file()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(io_error(path, source)),
    }
}

fn partial_path(destination: &Path) -> PathBuf {
    let mut name: OsString = destination.as_os_str().to_owned();
    name.push(".part");
    PathBuf::from(name)
}

fn backup_path(destination: &Path) -> PathBuf {
    let mut name: OsString = destination.as_os_str().to_owned();
    name.push(".backup");
    PathBuf::from(name)
}

fn io_error(path: &Path, source: std::io::Error) -> DownloadError {
    DownloadError::Io {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Mutex},
        time::{SystemTime, UNIX_EPOCH},
    };

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        time::sleep,
    };

    use super::*;

    #[tokio::test]
    async fn follows_redirect_and_reports_progress() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let body = vec![b'x'; 32 * 1024];
        let expected = body.clone();

        tokio::spawn(async move {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = vec![0_u8; 1024];
                let count = stream.read(&mut request).await.unwrap();
                let request = String::from_utf8_lossy(&request[..count]);

                if request.starts_with("GET /redirect ") {
                    stream
                        .write_all(
                            b"HTTP/1.1 302 Found\r\nLocation: /file\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        )
                        .await
                        .unwrap();
                } else {
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    stream.write_all(header.as_bytes()).await.unwrap();
                    for chunk in body.chunks(4096) {
                        stream.write_all(chunk).await.unwrap();
                        sleep(Duration::from_millis(8)).await;
                    }
                }
            }
        });

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let destination = std::env::temp_dir().join(format!(
            "eagle-downloader-{}-{unique}.bin",
            std::process::id()
        ));
        let events = Arc::new(Mutex::new(Vec::new()));
        let event_sink = Arc::clone(&events);
        let downloader = Downloader::builder()
            .progress_interval(Duration::from_millis(10))
            .build()
            .unwrap();

        let report = downloader
            .download(
                DownloadRequest::new(format!("http://{address}/redirect"), &destination),
                move |event| event_sink.lock().unwrap().push(event),
            )
            .await
            .unwrap();

        assert_eq!(fs::read(&destination).await.unwrap(), expected);
        assert_eq!(report.downloaded_bytes, 32 * 1024);
        assert!(report.final_url.ends_with("/file"));

        {
            let events = events.lock().unwrap();
            assert!(matches!(
                events.first(),
                Some(DownloadEvent::Started { .. })
            ));
            assert!(events
                .iter()
                .any(|event| matches!(event, DownloadEvent::Progress(progress) if progress.percentage.is_some())));
            assert!(matches!(events.last(), Some(DownloadEvent::Finished(_))));
        }

        fs::remove_file(destination).await.unwrap();
    }

    #[tokio::test]
    async fn recovers_a_stale_backup_before_replacing_destination() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let destination = std::env::temp_dir().join(format!(
            "eagle-downloader-replace-{}-{unique}.bin",
            std::process::id()
        ));
        let partial = partial_path(&destination);
        let backup = backup_path(&destination);
        fs::write(&backup, b"old").await.unwrap();
        fs::write(&partial, b"new").await.unwrap();

        replace_destination(&partial, &destination).await.unwrap();

        assert_eq!(fs::read(&destination).await.unwrap(), b"new");
        assert!(!file_exists(&backup).await.unwrap());
        fs::remove_file(destination).await.unwrap();
    }
}
