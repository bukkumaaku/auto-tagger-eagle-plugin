use protocol::{EchoRequest, EchoResult};

use super::HandlerResult;

pub fn handle(request: EchoRequest) -> HandlerResult<EchoResult> {
    Ok(EchoResult {
        message: request.message,
    })
}
