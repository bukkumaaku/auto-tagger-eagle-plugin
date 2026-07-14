use std::io::{BufRead, Write};

use protocol::{Request, Response};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Json error: {0}")]
    Json(#[from] serde_json::Error),
}

pub trait Transport {
    fn receive(&mut self) -> Result<Option<Request>, TransportError>;

    fn send(&mut self, response: &Response) -> Result<(), TransportError>;
}

pub struct JsonlTransport<R, W> {
    reader: R,
    writer: W,
    line: String,
}

impl<R, W> JsonlTransport<R, W> {
    pub fn new(reader: R, writer: W) -> Self {
        Self {
            reader,
            writer,
            line: String::new(),
        }
    }
}

impl<R, W> Transport for JsonlTransport<R, W>
where
    R: BufRead,
    W: Write,
{
    fn receive(&mut self) -> Result<Option<Request>, TransportError> {
        // 读取一行
        // 空行跳过
        // EOF 返回 Ok(None)
        // JSON 解析成 Request
        loop {
            self.line.clear();

            let bytes_read = self.reader.read_line(&mut self.line)?;
            if bytes_read == 0 {
                return Ok(None);
            }

            if self.line.trim().is_empty() {
                continue;
            }

            let request: Request = serde_json::from_str(self.line.trim())?;
            return Ok(Some(request));
        }
    }

    fn send(&mut self, response: &Response) -> Result<(), TransportError> {
        // Response 序列化成 JSON
        // 写入 JSON
        // 写入换行符
        // flush
        let json = serde_json::to_string(response)?;
        self.writer.write_all(json.as_bytes())?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()?;
        Ok(())
    }
}

#[cfg(test)]
impl<R, W> JsonlTransport<R, W> {
    pub fn into_writer(self) -> W {
        self.writer
    }
}
