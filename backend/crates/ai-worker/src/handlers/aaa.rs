use protocol::{AaaRequest, AaaResult};

use super::{HandlerError, HandlerResult};

pub fn handle(request: AaaRequest) -> HandlerResult<AaaResult> {
    if request.value != "aaa" {
        return Err(HandlerError::new(
            "INVALID_AAA_INPUT",
            format!("expected `aaa`, got `{}`", request.value),
        ));
    }

    Ok(AaaResult {
        value: "bbb".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_bbb_for_aaa() {
        let result = handle(AaaRequest {
            value: "aaa".to_string(),
        })
        .unwrap();

        assert_eq!(result.value, "bbb");
    }

    #[test]
    fn rejects_other_values() {
        let error = handle(AaaRequest {
            value: "ccc".to_string(),
        })
        .unwrap_err();

        assert_eq!(error.code, "INVALID_AAA_INPUT");
    }
}
