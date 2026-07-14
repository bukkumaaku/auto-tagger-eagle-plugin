use protocol::{Response, error_codes};
use thiserror::Error;

use crate::{
    dispatch,
    handlers::{CommandHandler, EventEmitter, HandlerError, HandlerResult},
    transport::{Transport, TransportError},
};

#[derive(Debug, Error)]
pub enum RunnerError {
    #[error(transparent)]
    Transport(#[from] TransportError),

    #[error("worker shutdown failed: {0}")]
    Shutdown(#[from] crate::handlers::HandlerError),
}

pub fn run<T: Transport, H: CommandHandler>(
    transport: &mut T,
    handlers: &mut H,
) -> Result<(), RunnerError> {
    let run_result = run_loop(transport, handlers);
    let shutdown_result = handlers.shutdown().map_err(RunnerError::Shutdown);

    match (run_result, shutdown_result) {
        (Err(error), _) => Err(error),
        (Ok(()), result) => result,
    }
}

fn run_loop<T: Transport, H: CommandHandler>(
    transport: &mut T,
    handlers: &mut H,
) -> Result<(), RunnerError> {
    loop {
        let request = match transport.receive() {
            Ok(Some(request)) => request,

            Ok(None) => return Ok(()),

            Err(TransportError::Json(error)) => {
                let response = Response::error(
                    None,
                    error_codes::BAD_MESSAGE,
                    format!("Failed to parse JSON: {error}"),
                );

                transport.send(&response)?;
                continue;
            }

            Err(error) => return Err(error.into()),
        };

        let request_id = request.request_id.clone();
        let response = {
            let mut events = TransportEventEmitter {
                request_id,
                transport,
            };
            dispatch::dispatch(request, handlers, &mut events)
        };

        transport.send(&response)?;
    }
}

struct TransportEventEmitter<'a, T> {
    request_id: String,
    transport: &'a mut T,
}

impl<T: Transport> EventEmitter for TransportEventEmitter<'_, T> {
    fn progress(&mut self, payload: protocol::ProgressPayload) -> HandlerResult<()> {
        self.transport
            .send(&Response::progress(self.request_id.clone(), payload))
            .map_err(|error| HandlerError::new("EVENT_SEND_FAILED", error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::{handlers::BuiltinHandlers, transport::JsonlTransport};

    use super::*;

    #[test]
    fn bad_json_does_not_stop_runner() {
        let input = concat!(
            "{bad-json}\n",
            "{\"protocolVersion\":1,\"requestId\":\"r1\",",
            "\"type\":\"echo\",\"payload\":{\"message\":\"hello\"}}\n"
        );

        let reader = Cursor::new(input.as_bytes());
        let writer = Vec::<u8>::new();
        let mut transport = JsonlTransport::new(reader, writer);
        let mut handlers = BuiltinHandlers::default();

        run(&mut transport, &mut handlers).unwrap();

        let output = String::from_utf8(transport.into_writer()).unwrap();
        let lines: Vec<&str> = output.lines().collect();

        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("BAD_MESSAGE"));
        assert!(lines[1].contains("hello"));
    }
}
