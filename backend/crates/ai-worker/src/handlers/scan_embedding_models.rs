use std::{fs, io, path::PathBuf};

use clip_embedding::EMBEDDING_DIMENSION;
use protocol::{EmbeddingModelInfo, ScanEmbeddingModelsRequest, ScanEmbeddingModelsResult};

use super::{HandlerError, HandlerResult};

pub fn handle(request: ScanEmbeddingModelsRequest) -> HandlerResult<ScanEmbeddingModelsResult> {
    if request.root.trim().is_empty() {
        return Err(HandlerError::new(
            "MODEL_ROOT_EMPTY",
            "model root cannot be empty",
        ));
    }

    let models_directory = PathBuf::from(&request.root).join("embedding");
    if !models_directory.is_dir() {
        return Ok(ScanEmbeddingModelsResult {
            models_directory: models_directory.to_string_lossy().into_owned(),
            models: Vec::new(),
        });
    }

    let entries = fs::read_dir(&models_directory).map_err(|error| {
        HandlerError::new(
            "MODEL_SCAN_FAILED",
            format!(
                "failed to read embedding model directory {}: {error}",
                models_directory.display()
            ),
        )
    })?;
    let mut models = Vec::new();
    for entry in entries {
        let directory = entry.map_err(directory_entry_error)?.path();
        if !directory.is_dir() {
            continue;
        }
        let model_path = directory.join("onnx").join("model_quantized.onnx");
        let tokenizer_path = directory.join("tokenizer.json");
        if !model_path.is_file() || !tokenizer_path.is_file() {
            continue;
        }
        let Some(name) = directory.file_name() else {
            continue;
        };
        let name = name.to_string_lossy().into_owned();
        models.push(EmbeddingModelInfo {
            model_key: name.clone(),
            name,
            model_path: model_path.to_string_lossy().into_owned(),
            tokenizer_path: tokenizer_path.to_string_lossy().into_owned(),
            dimension: EMBEDDING_DIMENSION,
        });
    }
    models.sort_by_key(|model| model.name.to_lowercase());

    Ok(ScanEmbeddingModelsResult {
        models_directory: models_directory.to_string_lossy().into_owned(),
        models,
    })
}

fn directory_entry_error(error: io::Error) -> HandlerError {
    HandlerError::new(
        "MODEL_SCAN_FAILED",
        format!("failed to inspect embedding model directory entry: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn finds_complete_embedding_models_only() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("embedding-model-scan-{suffix}"));
        let complete = root.join("embedding").join("jina-clip-v2-q8");
        let incomplete = root.join("embedding").join("incomplete");
        fs::create_dir_all(complete.join("onnx")).unwrap();
        fs::create_dir_all(&incomplete).unwrap();
        fs::write(complete.join("onnx").join("model_quantized.onnx"), []).unwrap();
        fs::write(complete.join("tokenizer.json"), []).unwrap();
        fs::write(incomplete.join("tokenizer.json"), []).unwrap();

        let result = handle(ScanEmbeddingModelsRequest {
            root: root.to_string_lossy().into_owned(),
        })
        .unwrap();

        assert_eq!(result.models.len(), 1);
        assert_eq!(result.models[0].model_key, "jina-clip-v2-q8");
        assert_eq!(result.models[0].dimension, EMBEDDING_DIMENSION);
        fs::remove_dir_all(root).unwrap();
    }
}
