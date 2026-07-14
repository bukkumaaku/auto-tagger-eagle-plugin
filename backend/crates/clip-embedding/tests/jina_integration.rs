use std::{env, path::PathBuf, time::SystemTime};

use clip_embedding::{ClipConfig, EMBEDDING_DIMENSION, ExecutionProvider, JinaClip};
use vector_store::{Modality, VectorRecord, VectorStore};

#[test]
#[ignore = "requires Jina CLIP model files and DirectML"]
fn directml_text_and_image_embeddings_are_searchable() {
    let model_path = required_path("JINA_CLIP_MODEL");
    let tokenizer_path = required_path("JINA_CLIP_TOKENIZER");
    let image_directory = required_path("JINA_CLIP_TEST_IMAGES");
    let mut image_paths = std::fs::read_dir(&image_directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    matches!(
                        extension.to_ascii_lowercase().as_str(),
                        "png" | "jpg" | "jpeg" | "webp" | "bmp"
                    )
                })
        })
        .collect::<Vec<_>>();
    image_paths.sort();
    assert!(!image_paths.is_empty(), "test image directory is empty");

    let mut config = ClipConfig::new(model_path, tokenizer_path);
    config.execution_provider = ExecutionProvider::DirectMl;
    let mut clip = JinaClip::load(config).unwrap();
    let image_embeddings = clip.embed_images(&image_paths).unwrap();
    let text_embedding = clip.embed_text("博物馆").unwrap();

    assert_normalized(&text_embedding);
    for embedding in &image_embeddings {
        assert_normalized(embedding);
    }

    let mut store = VectorStore::open_in_memory("jina-clip-v2-q8", EMBEDDING_DIMENSION).unwrap();
    let updated_at = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let records = image_paths
        .iter()
        .zip(image_embeddings)
        .enumerate()
        .map(|(index, (path, embedding))| VectorRecord {
            namespace: "integration".to_string(),
            item_id: format!("image-{index}"),
            modality: Modality::Image,
            source_key: "primary".to_string(),
            source_uri: Some(path.to_string_lossy().into_owned()),
            content: None,
            updated_at,
            embedding,
        })
        .collect::<Vec<_>>();
    store.upsert_many(&records).unwrap();

    let results = store
        .search("integration", Some(Modality::Image), &text_embedding, 5)
        .unwrap();
    for (rank, result) in results.iter().enumerate() {
        println!(
            "rank={} similarity={:.4} path={}",
            rank + 1,
            result.similarity,
            result.source_uri.as_deref().unwrap_or_default()
        );
    }
    assert!(!results.is_empty());
    let first_path = results[0].source_uri.as_deref().unwrap_or_default();
    assert!(
        first_path.to_ascii_lowercase().contains("museum"),
        "expected a museum image first, got {first_path}; results={results:?}"
    );
}

fn required_path(name: &str) -> PathBuf {
    env::var_os(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("environment variable {name} is required"))
}

fn assert_normalized(embedding: &[f32]) {
    assert_eq!(embedding.len(), EMBEDDING_DIMENSION);
    assert!(embedding.iter().all(|value| value.is_finite()));
    let norm = embedding
        .iter()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    assert!((norm - 1.0).abs() < 1e-4, "embedding norm is {norm}");
}
