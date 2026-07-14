use std::{fs, io, path::Path};

use protocol::{PathInfoRequest, PathInfoResult};

use super::{HandlerError, HandlerResult};

pub fn handle(request: PathInfoRequest) -> HandlerResult<PathInfoResult> {
    let path = Path::new(&request.path);

    match fs::metadata(path) {
        Ok(metadata) => Ok(PathInfoResult {
            path: request.path,
            exists: true,
            is_file: metadata.is_file(),
            is_directory: metadata.is_dir(),
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(PathInfoResult {
            path: request.path,
            exists: false,
            is_file: false,
            is_directory: false,
        }),
        Err(error) => Err(HandlerError::new(
            "PATH_METADATA_FAILED",
            format!("failed to inspect {}: {error}", path.display()),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_existing_file() {
        let result = handle(PathInfoRequest {
            path: std::env::current_exe()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
        })
        .unwrap();

        assert!(result.exists);
        assert!(result.is_file);
        assert!(!result.is_directory);
    }

    #[test]
    fn reports_missing_path() {
        let result = handle(PathInfoRequest {
            path: "path-that-does-not-exist-for-ai-worker-test".to_string(),
        })
        .unwrap();

        assert!(!result.exists);
        assert!(!result.is_file);
        assert!(!result.is_directory);
    }
}
