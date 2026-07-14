use std::{fs, io, path::PathBuf};

use protocol::{ScanWdModelsRequest, ScanWdModelsResult, WdModelInfo, WdModelKind};

use super::{HandlerError, HandlerResult};

pub fn handle(request: ScanWdModelsRequest) -> HandlerResult<ScanWdModelsResult> {
    if request.root.trim().is_empty() {
        return Err(HandlerError::new(
            "MODEL_ROOT_EMPTY",
            "model root cannot be empty",
        ));
    }

    let root = PathBuf::from(&request.root);
    let models_directory = root.join("wd");
    if !models_directory.is_dir() {
        return Ok(ScanWdModelsResult {
            models_directory: models_directory.to_string_lossy().into_owned(),
            models: Vec::new(),
        });
    }
    let entries = fs::read_dir(&models_directory).map_err(|error| {
        HandlerError::new(
            "MODEL_SCAN_FAILED",
            format!(
                "failed to read model directory {}: {error}",
                models_directory.display()
            ),
        )
    })?;
    let mut models = Vec::new();

    for entry in entries {
        let entry = entry.map_err(directory_entry_error)?;
        let directory = entry.path();
        if !directory.is_dir() {
            continue;
        }
        if let Some(model) = identify_model(directory) {
            models.push(model);
        }
    }

    models.sort_by_key(|model| model.name.to_lowercase());
    Ok(ScanWdModelsResult {
        models_directory: models_directory.to_string_lossy().into_owned(),
        models,
    })
}

fn identify_model(directory: PathBuf) -> Option<WdModelInfo> {
    let camie_tags = directory.join("camie-tagger-v2-metadata.json");
    if camie_tags.is_file() {
        let model_path = [
            directory.join("model.onnx"),
            directory.join("camie-tagger-v2.onnx"),
        ]
        .into_iter()
        .find(|path| path.is_file())?;
        return model_info(directory, WdModelKind::Camie, model_path, camie_tags);
    }

    let model_path = directory.join("model.onnx");
    if !model_path.is_file() {
        return None;
    }
    let candidates = [
        (WdModelKind::Wd, directory.join("selected_tags.csv")),
        (WdModelKind::Cl, directory.join("tag_mapping.json")),
    ];
    let (model_kind, tags_path) = candidates
        .into_iter()
        .find(|(_, tags_path)| tags_path.is_file())?;

    model_info(directory, model_kind, model_path, tags_path)
}

fn model_info(
    directory: PathBuf,
    model_kind: WdModelKind,
    model_path: PathBuf,
    tags_path: PathBuf,
) -> Option<WdModelInfo> {
    Some(WdModelInfo {
        name: directory.file_name()?.to_string_lossy().into_owned(),
        model_kind,
        model_path: model_path.to_string_lossy().into_owned(),
        tags_path: tags_path.to_string_lossy().into_owned(),
    })
}

fn directory_entry_error(error: io::Error) -> HandlerError {
    HandlerError::new(
        "MODEL_SCAN_FAILED",
        format!("failed to inspect model directory entry: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn rejects_empty_model_root() {
        let error = handle(ScanWdModelsRequest {
            root: " ".to_string(),
        })
        .unwrap_err();

        assert_eq!(error.code, "MODEL_ROOT_EMPTY");
    }

    #[test]
    fn uses_wd_directory_under_model_root() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("ai-worker-model-scan-{suffix}"));
        fs::create_dir_all(root.join("wd").join("example-model")).unwrap();
        fs::write(root.join("wd").join("example-model").join("model.onnx"), []).unwrap();
        fs::write(
            root.join("wd")
                .join("example-model")
                .join("selected_tags.csv"),
            [],
        )
        .unwrap();

        let result = handle(ScanWdModelsRequest {
            root: root.to_string_lossy().into_owned(),
        })
        .unwrap();

        assert_eq!(PathBuf::from(result.models_directory), root.join("wd"));
        assert_eq!(result.models.len(), 1);
        assert_eq!(result.models[0].name, "example-model");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_wd_directory_returns_empty_result() {
        let root = std::env::temp_dir().join("ai-worker-missing-wd-root");
        let result = handle(ScanWdModelsRequest {
            root: root.to_string_lossy().into_owned(),
        })
        .unwrap();

        assert_eq!(PathBuf::from(result.models_directory), root.join("wd"));
        assert!(result.models.is_empty());
    }
}
