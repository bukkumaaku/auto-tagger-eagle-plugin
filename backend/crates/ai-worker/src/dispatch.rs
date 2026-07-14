use crate::handlers::{CommandHandler, EventEmitter};
use protocol::{PROTOCOL_VERSION, Request, Response, error_codes};

pub fn dispatch<H: CommandHandler>(
    request: Request,
    handlers: &mut H,
    events: &mut dyn EventEmitter,
) -> Response {
    let Request {
        protocol_version,
        command,
        request_id,
    } = request;

    if protocol_version != PROTOCOL_VERSION {
        return Response::error(
            Some(request_id),
            error_codes::INVALID_PROTOCOL_VERSION,
            format!(
                "Expected protocol version {}, but got {}",
                PROTOCOL_VERSION, protocol_version
            ),
        );
    }

    match handlers.handle(command, events) {
        Ok(result) => Response::result(request_id, result),
        Err(error) => Response::error(Some(request_id), error.code, error.message),
    }
}

#[cfg(test)]
mod tests {
    use protocol::{Command, EchoRequest, ErrorPayload, ResponseMessage, ResultPayload};

    use crate::handlers::{HandlerError, HandlerResult};

    use super::*;

    struct FailingHandler;

    struct NoopEvents;

    impl EventEmitter for NoopEvents {
        fn progress(&mut self, _payload: protocol::ProgressPayload) -> HandlerResult<()> {
            Ok(())
        }
    }

    impl CommandHandler for FailingHandler {
        fn handle(
            &mut self,
            _command: Command,
            _events: &mut dyn EventEmitter,
        ) -> HandlerResult<ResultPayload> {
            Err(HandlerError::new("FEATURE_FAILED", "feature failed"))
        }
    }

    fn echo_request(protocol_version: u32) -> Request {
        Request {
            protocol_version,
            request_id: "r1".to_string(),
            command: Command::Echo(EchoRequest {
                message: "hello".to_string(),
            }),
        }
    }

    #[test]
    fn wraps_handler_errors_in_protocol_response() {
        let response = dispatch(
            echo_request(PROTOCOL_VERSION),
            &mut FailingHandler,
            &mut NoopEvents,
        );

        assert_eq!(response.request_id.as_deref(), Some("r1"));
        match response.message {
            ResponseMessage::Error(ErrorPayload { code, message }) => {
                assert_eq!(code, "FEATURE_FAILED");
                assert_eq!(message, "feature failed");
            }
            _ => panic!("expected an error response"),
        }
    }

    #[test]
    fn rejects_unsupported_protocol_version_before_handler() {
        let response = dispatch(
            echo_request(PROTOCOL_VERSION + 1),
            &mut FailingHandler,
            &mut NoopEvents,
        );

        match response.message {
            ResponseMessage::Error(ErrorPayload { code, .. }) => {
                assert_eq!(code, error_codes::INVALID_PROTOCOL_VERSION);
            }
            _ => panic!("expected a protocol version error"),
        }
    }
}
