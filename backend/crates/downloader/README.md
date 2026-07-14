# downloader

异步文件下载库，自动处理最多 10 次 HTTP 重定向，并默认每 500ms 上报一次进度。

```rust
use std::path::PathBuf;

use downloader::{DownloadEvent, DownloadRequest, Downloader};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let downloader = Downloader::new()?;
    let request = DownloadRequest::new(
        "https://example.com/model.gguf",
        PathBuf::from("models/model.gguf"),
    );

    let report = downloader
        .download(request, |event| match event {
            DownloadEvent::Started { total_bytes, .. } => {
                println!("total: {total_bytes:?}");
            }
            DownloadEvent::Progress(progress) => {
                println!(
                    "{:?}% | {:.2} MiB/s | remaining {:?} bytes",
                    progress.percentage,
                    progress.bytes_per_second / 1024.0 / 1024.0,
                    progress.remaining_bytes,
                );
            }
            DownloadEvent::Finished(report) => {
                println!("saved to {}", report.destination.display());
            }
        })
        .await?;

    println!("downloaded {} bytes", report.downloaded_bytes);
    Ok(())
}
```

`total_bytes`、`remaining_bytes` 和 `percentage` 在服务器没有返回
`Content-Length` 时为 `None`；下载完成后，最终报告会使用实际字节数作为总大小。

下载先写入 `<destination>.part`。成功后才替换目标文件；失败时删除临时文件。
回调在下载任务内同步执行，应只做轻量操作，例如把事件转发到 JSONL 通道。
