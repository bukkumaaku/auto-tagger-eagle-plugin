use std::{cmp::Ordering, fs, io, path::Path};

use image::{
    DynamicImage, ImageBuffer, Rgb, RgbImage,
    imageops::{FilterType, overlay, resize},
};
use ndarray::Array4;
#[cfg(target_os = "macos")]
use ort::execution_providers::CoreMLExecutionProvider;
#[cfg(windows)]
use ort::execution_providers::DirectMLExecutionProvider;
use ort::{
    session::{Session, builder::GraphOptimizationLevel},
    value::TensorRef,
};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelKind {
    Wd,
    Cl,
    Camie,
}

impl ModelKind {
    fn image_size(self) -> u32 {
        match self {
            Self::Wd | Self::Cl => 448,
            Self::Camie => 512,
        }
    }

    fn uses_nhwc(self) -> bool {
        matches!(self, Self::Wd)
    }

    fn output_is_logits(self) -> bool {
        matches!(self, Self::Cl | Self::Camie)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionProvider {
    Auto,
    DirectMl,
    Cpu,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TagScore {
    pub name: String,
    pub score: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImagePrediction {
    pub path: String,
    pub tags: Vec<TagScore>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImageFailure {
    pub index: usize,
    pub path: String,
    pub error: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BatchPrediction {
    pub predictions: Vec<ImagePrediction>,
    pub failures: Vec<ImageFailure>,
}

#[derive(Debug, Error)]
pub enum TaggerError {
    #[error("model file does not exist: {0}")]
    ModelNotFound(String),

    #[error("tag metadata file does not exist: {0}")]
    TagsNotFound(String),

    #[error("failed to initialize ONNX Runtime: {0}")]
    Ort(#[from] ort::Error),

    #[error("execution provider `{0}` is not supported on this platform")]
    UnsupportedExecutionProvider(&'static str),

    #[error("failed to open image {path}: {source}")]
    OpenImage {
        path: String,
        source: image::ImageError,
    },

    #[error("failed to read image {path}: {source}")]
    ReadImage { path: String, source: io::Error },

    #[error("GIF images are not supported: {0}")]
    GifNotSupported(String),

    #[error("failed to read tag metadata {path}: {source}")]
    ReadTags { path: String, source: io::Error },

    #[error("failed to parse WD tag CSV {path}: {source}")]
    ParseCsv { path: String, source: csv::Error },

    #[error("failed to parse tag JSON {path}: {source}")]
    ParseJson {
        path: String,
        source: serde_json::Error,
    },

    #[error("tag metadata has an unsupported structure: {0}")]
    InvalidTags(String),

    #[error("cannot run inference with an empty image list")]
    EmptyBatch,

    #[error("threshold must be between 0 and 1, got {0}")]
    InvalidThreshold(f32),

    #[error("model output contains {output_count} values for {batch_size} images")]
    InvalidOutputShape {
        output_count: usize,
        batch_size: usize,
    },

    #[error("model returned {actual} scores per image, but metadata contains {expected} tags")]
    TagCountMismatch { actual: usize, expected: usize },
}

pub struct WdTagger {
    session: Session,
    tags: Vec<String>,
    model_path: String,
    tags_path: String,
    model_kind: ModelKind,
    execution_provider: ExecutionProvider,
}

impl WdTagger {
    pub fn load(
        model_path: impl Into<String>,
        tags_path: impl Into<String>,
        model_kind: ModelKind,
        execution_provider: ExecutionProvider,
    ) -> Result<Self, TaggerError> {
        let model_path = model_path.into();
        let tags_path = tags_path.into();

        if !Path::new(&model_path).is_file() {
            return Err(TaggerError::ModelNotFound(model_path));
        }
        if !Path::new(&tags_path).is_file() {
            return Err(TaggerError::TagsNotFound(tags_path));
        }

        let tags = load_tags(&tags_path, model_kind)?;
        let session = create_session(&model_path, execution_provider)?;

        Ok(Self {
            session,
            tags,
            model_path,
            tags_path,
            model_kind,
            execution_provider,
        })
    }

    pub fn model_path(&self) -> &str {
        &self.model_path
    }

    pub fn tags_path(&self) -> &str {
        &self.tags_path
    }

    pub fn model_kind(&self) -> ModelKind {
        self.model_kind
    }

    pub fn execution_provider(&self) -> ExecutionProvider {
        self.execution_provider
    }

    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }

    pub fn predict(
        &mut self,
        image_paths: &[String],
        threshold: f32,
    ) -> Result<Vec<ImagePrediction>, TaggerError> {
        validate_batch(image_paths, threshold)?;
        let values = image_paths
            .iter()
            .map(|path| decode_image(path).map(|image| prepare_image(image, self.model_kind)))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect();
        self.predict_preprocessed(image_paths, values, threshold)
    }

    pub fn predict_partial(
        &mut self,
        image_paths: &[String],
        threshold: f32,
    ) -> Result<BatchPrediction, TaggerError> {
        validate_batch(image_paths, threshold)?;
        let mut valid_indices = Vec::new();
        let mut values = Vec::new();
        let mut failures = Vec::new();

        for (index, path) in image_paths.iter().enumerate() {
            match decode_image(path) {
                Ok(image) => {
                    valid_indices.push(index);
                    values.extend(prepare_image(image, self.model_kind));
                }
                Err(error) => failures.push(ImageFailure {
                    index,
                    path: path.clone(),
                    error: error.to_string(),
                }),
            }
        }

        let mut predictions = image_paths
            .iter()
            .map(|path| ImagePrediction {
                path: path.clone(),
                tags: Vec::new(),
            })
            .collect::<Vec<_>>();
        if !valid_indices.is_empty() {
            let valid_paths = valid_indices
                .iter()
                .map(|index| image_paths[*index].clone())
                .collect::<Vec<_>>();
            let valid_predictions = self.predict_preprocessed(&valid_paths, values, threshold)?;
            for (index, prediction) in valid_indices.into_iter().zip(valid_predictions) {
                predictions[index] = prediction;
            }
        }

        Ok(BatchPrediction {
            predictions,
            failures,
        })
    }

    fn predict_preprocessed(
        &mut self,
        image_paths: &[String],
        values: Vec<f32>,
        threshold: f32,
    ) -> Result<Vec<ImagePrediction>, TaggerError> {
        let input = build_batch(image_paths.len(), values, self.model_kind);
        let outputs = self
            .session
            .run(ort::inputs![TensorRef::from_array_view(&input)?])?;
        let (_, output) = outputs[0].try_extract_tensor::<f32>()?;

        if output.len() % image_paths.len() != 0 {
            return Err(TaggerError::InvalidOutputShape {
                output_count: output.len(),
                batch_size: image_paths.len(),
            });
        }

        let scores_per_image = output.len() / image_paths.len();
        if scores_per_image != self.tags.len() {
            return Err(TaggerError::TagCountMismatch {
                actual: scores_per_image,
                expected: self.tags.len(),
            });
        }

        Ok(image_paths
            .iter()
            .zip(output.chunks_exact(scores_per_image))
            .map(|(path, scores)| ImagePrediction {
                path: path.clone(),
                tags: collect_tags(&self.tags, scores, threshold, self.model_kind),
            })
            .collect())
    }
}

fn validate_batch(image_paths: &[String], threshold: f32) -> Result<(), TaggerError> {
    if image_paths.is_empty() {
        return Err(TaggerError::EmptyBatch);
    }
    if !(0.0..=1.0).contains(&threshold) {
        return Err(TaggerError::InvalidThreshold(threshold));
    }
    Ok(())
}

fn create_session(
    model_path: &str,
    execution_provider: ExecutionProvider,
) -> Result<Session, TaggerError> {
    let builder = Session::builder()?
        .with_optimization_level(GraphOptimizationLevel::Level3)?
        .with_parallel_execution(true)?;

    let builder = match execution_provider {
        ExecutionProvider::Cpu => builder,
        ExecutionProvider::Auto => {
            #[cfg(windows)]
            {
                builder.with_execution_providers([DirectMLExecutionProvider::default()
                    .build()
                    .fail_silently()])?
            }
            #[cfg(target_os = "macos")]
            {
                builder.with_execution_providers([CoreMLExecutionProvider::default()
                    .build()
                    .fail_silently()])?
            }
            #[cfg(not(any(windows, target_os = "macos")))]
            {
                builder
            }
        }
        ExecutionProvider::DirectMl => {
            #[cfg(windows)]
            {
                builder.with_execution_providers([DirectMLExecutionProvider::default()
                    .build()
                    .error_on_failure()])?
            }
            #[cfg(not(windows))]
            {
                return Err(TaggerError::UnsupportedExecutionProvider("direct_ml"));
            }
        }
    };

    Ok(builder.commit_from_file(model_path)?)
}

fn build_batch(batch_size: usize, values: Vec<f32>, model_kind: ModelKind) -> Array4<f32> {
    let size = model_kind.image_size() as usize;
    let shape = if model_kind.uses_nhwc() {
        (batch_size, size, size, 3)
    } else {
        (batch_size, 3, size, size)
    };

    Array4::from_shape_vec(shape, values).expect("preprocessed image shape is fixed")
}

fn decode_image(path: &str) -> Result<DynamicImage, TaggerError> {
    let bytes = fs::read(path).map_err(|source| TaggerError::ReadImage {
        path: path.to_string(),
        source,
    })?;
    let format = image::guess_format(&bytes).map_err(|source| TaggerError::OpenImage {
        path: path.to_string(),
        source,
    })?;
    if format == image::ImageFormat::Gif {
        return Err(TaggerError::GifNotSupported(path.to_string()));
    }
    image::load_from_memory_with_format(&bytes, format).map_err(|source| TaggerError::OpenImage {
        path: path.to_string(),
        source,
    })
}

fn prepare_image(image: DynamicImage, model_kind: ModelKind) -> Vec<f32> {
    let image = flatten_alpha_on_black(image);
    let image = contain_on_white(&image, model_kind.image_size());

    match model_kind {
        ModelKind::Wd => image
            .pixels()
            .flat_map(|pixel| [pixel[2] as f32, pixel[1] as f32, pixel[0] as f32])
            .collect(),
        ModelKind::Cl => planar_bgr(&image, [0.5; 3], [0.5; 3]),
        ModelKind::Camie => planar_bgr(&image, [0.485, 0.456, 0.406], [0.229, 0.224, 0.225]),
    }
}

fn flatten_alpha_on_black(image: DynamicImage) -> RgbImage {
    let rgba = image.to_rgba8();
    ImageBuffer::from_fn(rgba.width(), rgba.height(), |x, y| {
        let pixel = rgba.get_pixel(x, y);
        let alpha = u16::from(pixel[3]);
        Rgb([
            ((u16::from(pixel[0]) * alpha + 127) / 255) as u8,
            ((u16::from(pixel[1]) * alpha + 127) / 255) as u8,
            ((u16::from(pixel[2]) * alpha + 127) / 255) as u8,
        ])
    })
}

fn contain_on_white(image: &RgbImage, size: u32) -> RgbImage {
    let (width, height) = image.dimensions();
    let scale = (size as f64 / width as f64).min(size as f64 / height as f64);
    let resized_width = ((width as f64 * scale).round() as u32).clamp(1, size);
    let resized_height = ((height as f64 * scale).round() as u32).clamp(1, size);
    let resized = resize(image, resized_width, resized_height, FilterType::Lanczos3);
    let mut canvas = ImageBuffer::from_pixel(size, size, Rgb([255, 255, 255]));
    overlay(
        &mut canvas,
        &resized,
        i64::from((size - resized_width) / 2),
        i64::from((size - resized_height) / 2),
    );
    canvas
}

fn planar_bgr(image: &RgbImage, mean: [f32; 3], std: [f32; 3]) -> Vec<f32> {
    let plane_size = (image.width() * image.height()) as usize;
    let mut values = vec![0.0; plane_size * 3];

    for (index, pixel) in image.pixels().enumerate() {
        values[index] = (f32::from(pixel[2]) / 255.0 - mean[2]) / std[2];
        values[plane_size + index] = (f32::from(pixel[1]) / 255.0 - mean[1]) / std[1];
        values[plane_size * 2 + index] = (f32::from(pixel[0]) / 255.0 - mean[0]) / std[0];
    }

    values
}

fn collect_tags(
    tags: &[String],
    scores: &[f32],
    threshold: f32,
    model_kind: ModelKind,
) -> Vec<TagScore> {
    let mut result = tags
        .iter()
        .zip(scores)
        .filter_map(|(name, score)| {
            let score = if model_kind.output_is_logits() {
                sigmoid(*score)
            } else {
                *score
            };
            (score > threshold).then(|| TagScore {
                name: name.clone(),
                score,
            })
        })
        .collect::<Vec<_>>();

    result.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(Ordering::Equal)
    });
    result
}

fn sigmoid(value: f32) -> f32 {
    if value.is_nan() {
        return 0.0;
    }
    1.0 / (1.0 + (-value.clamp(-30.0, 30.0)).exp())
}

fn load_tags(path: &str, model_kind: ModelKind) -> Result<Vec<String>, TaggerError> {
    match model_kind {
        ModelKind::Wd => load_wd_tags(path),
        ModelKind::Cl => load_cl_tags(path),
        ModelKind::Camie => load_camie_tags(path),
    }
}

fn load_wd_tags(path: &str) -> Result<Vec<String>, TaggerError> {
    let mut reader = csv::Reader::from_path(path).map_err(|source| TaggerError::ParseCsv {
        path: path.to_string(),
        source,
    })?;
    let headers = reader
        .headers()
        .map_err(|source| TaggerError::ParseCsv {
            path: path.to_string(),
            source,
        })?
        .clone();
    let name_index = headers
        .iter()
        .position(|header| header == "name")
        .unwrap_or(1);
    let mut tags = Vec::new();

    for record in reader.records() {
        let record = record.map_err(|source| TaggerError::ParseCsv {
            path: path.to_string(),
            source,
        })?;
        let name = record
            .get(name_index)
            .ok_or_else(|| TaggerError::InvalidTags(path.to_string()))?;
        tags.push(name.to_string());
    }

    require_tags(path, tags)
}

fn load_cl_tags(path: &str) -> Result<Vec<String>, TaggerError> {
    let value = read_json(path)?;
    let entries = value
        .as_array()
        .map(|items| items.iter().collect::<Vec<_>>())
        .or_else(|| {
            value.as_object().map(|items| {
                let mut entries = items.iter().collect::<Vec<_>>();
                entries.sort_by_key(|(key, _)| key.parse::<usize>().unwrap_or(usize::MAX));
                entries.into_iter().map(|(_, value)| value).collect()
            })
        })
        .ok_or_else(|| TaggerError::InvalidTags(path.to_string()))?;
    let tags = entries
        .into_iter()
        .map(|entry| {
            entry
                .get("tag")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| TaggerError::InvalidTags(path.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    require_tags(path, tags)
}

fn load_camie_tags(path: &str) -> Result<Vec<String>, TaggerError> {
    let value = read_json(path)?;
    let mapping = value
        .pointer("/dataset_info/tag_mapping/idx_to_tag")
        .ok_or_else(|| TaggerError::InvalidTags(path.to_string()))?;
    let tags = if let Some(items) = mapping.as_array() {
        items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| TaggerError::InvalidTags(path.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?
    } else if let Some(items) = mapping.as_object() {
        let mut entries = items.iter().collect::<Vec<_>>();
        entries.sort_by_key(|(key, _)| key.parse::<usize>().unwrap_or(usize::MAX));
        entries
            .into_iter()
            .map(|(_, item)| {
                item.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| TaggerError::InvalidTags(path.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        return Err(TaggerError::InvalidTags(path.to_string()));
    };
    require_tags(path, tags)
}

fn read_json(path: &str) -> Result<serde_json::Value, TaggerError> {
    let source = fs::read_to_string(path).map_err(|source| TaggerError::ReadTags {
        path: path.to_string(),
        source,
    })?;
    serde_json::from_str(&source).map_err(|source| TaggerError::ParseJson {
        path: path.to_string(),
        source,
    })
}

fn require_tags(path: &str, tags: Vec<String>) -> Result<Vec<String>, TaggerError> {
    if tags.is_empty() {
        Err(TaggerError::InvalidTags(path.to_string()))
    } else {
        Ok(tags)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wd_preprocessing_uses_nhwc_bgr() {
        let image = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(1, 1, Rgb([10, 20, 30])));
        let values = prepare_image(image, ModelKind::Wd);

        assert_eq!(values.len(), 448 * 448 * 3);
        assert_eq!(&values[..3], &[30.0, 20.0, 10.0]);
    }

    #[test]
    fn cl_logits_are_converted_and_sorted() {
        let tags = vec!["low".to_string(), "high".to_string()];
        let result = collect_tags(&tags, &[0.0, 2.0], 0.4, ModelKind::Cl);

        assert_eq!(result[0].name, "high");
        assert_eq!(result[1].name, "low");
        assert!(result[0].score > result[1].score);
    }

    #[test]
    fn transparent_pixels_are_flattened_on_black() {
        let image =
            DynamicImage::ImageRgba8(ImageBuffer::from_pixel(1, 1, image::Rgba([255, 0, 0, 0])));
        let flattened = flatten_alpha_on_black(image);

        assert_eq!(flattened.get_pixel(0, 0), &Rgb([0, 0, 0]));
    }
}
