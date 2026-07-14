//! Jina CLIP v2 image and text embeddings with DirectML-first ONNX inference.

use std::path::{Path, PathBuf};

use image::{DynamicImage, ImageFormat, RgbImage, imageops::FilterType};
use ndarray::{Array2, Array4};
#[cfg(target_os = "macos")]
use ort::execution_providers::CoreMLExecutionProvider;
#[cfg(windows)]
use ort::execution_providers::DirectMLExecutionProvider;
use ort::{
    session::{Session, builder::GraphOptimizationLevel},
    value::TensorRef,
};
use thiserror::Error;
use tokenizers::{Tokenizer, TruncationParams};

pub const EMBEDDING_DIMENSION: usize = 1024;
pub const IMAGE_SIZE: u32 = 512;
pub const QUERY_PREFIX: &str = "Represent the query for retrieving evidence documents: ";

const IMAGE_MEAN: [f32; 3] = [0.481_454_66, 0.457_827_5, 0.408_210_73];
const IMAGE_STD: [f32; 3] = [0.268_629_55, 0.261_302_6, 0.275_777_1];
const TEXT_OUTPUT: &str = "l2norm_text_embeddings";
const IMAGE_OUTPUT: &str = "l2norm_image_embeddings";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionProvider {
    Auto,
    DirectMl,
    Cpu,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClipConfig {
    pub model_path: PathBuf,
    pub tokenizer_path: PathBuf,
    pub execution_provider: ExecutionProvider,
    pub max_text_length: usize,
}

impl ClipConfig {
    pub fn new(model_path: impl Into<PathBuf>, tokenizer_path: impl Into<PathBuf>) -> Self {
        Self {
            model_path: model_path.into(),
            tokenizer_path: tokenizer_path.into(),
            execution_provider: ExecutionProvider::Auto,
            max_text_length: 8192,
        }
    }
}

#[derive(Debug, Error)]
pub enum ClipError {
    #[error("Jina CLIP model file does not exist: {0}")]
    ModelNotFound(PathBuf),

    #[error("Jina CLIP tokenizer file does not exist: {0}")]
    TokenizerNotFound(PathBuf),

    #[error("failed to load tokenizer: {0}")]
    Tokenizer(String),

    #[error("failed to initialize ONNX Runtime: {0}")]
    Ort(#[from] ort::Error),

    #[error("execution provider `{0}` is not supported on this platform")]
    UnsupportedExecutionProvider(&'static str),

    #[error("model is missing required input `{required}`; available inputs: {available:?}")]
    MissingInput {
        required: &'static str,
        available: Vec<String>,
    },

    #[error("model is missing required output `{required}`; available outputs: {available:?}")]
    MissingOutput {
        required: &'static str,
        available: Vec<String>,
    },

    #[error("text cannot be empty")]
    EmptyText,

    #[error("image batch cannot be empty")]
    EmptyImageBatch,

    #[error("failed to read image {path}: {source}")]
    ReadImage {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to decode image {path}: {source}")]
    DecodeImage {
        path: PathBuf,
        source: image::ImageError,
    },

    #[error("GIF images are not supported: {0}")]
    GifNotSupported(PathBuf),

    #[error("{kind} embedding output has {actual} values; expected {expected}")]
    InvalidOutputShape {
        kind: &'static str,
        actual: usize,
        expected: usize,
    },

    #[error("{kind} embedding contains non-finite values")]
    NonFiniteEmbedding { kind: &'static str },

    #[error("{kind} embedding has zero L2 norm")]
    ZeroNormEmbedding { kind: &'static str },
}

pub struct JinaClip {
    session: Session,
    tokenizer: Tokenizer,
    config: ClipConfig,
    has_attention_mask: bool,
}

impl JinaClip {
    pub fn load(config: ClipConfig) -> Result<Self, ClipError> {
        require_file(&config.model_path, ClipError::ModelNotFound)?;
        require_file(&config.tokenizer_path, ClipError::TokenizerNotFound)?;

        let mut tokenizer = Tokenizer::from_file(&config.tokenizer_path)
            .map_err(|error| ClipError::Tokenizer(error.to_string()))?;
        tokenizer
            .with_truncation(Some(TruncationParams {
                max_length: config.max_text_length,
                ..TruncationParams::default()
            }))
            .map_err(|error| ClipError::Tokenizer(error.to_string()))?;

        let session = create_session(&config.model_path, config.execution_provider)?;
        let input_names = session
            .inputs
            .iter()
            .map(|input| input.name.clone())
            .collect::<Vec<_>>();
        for required in ["input_ids", "pixel_values"] {
            if !input_names.iter().any(|name| name == required) {
                return Err(ClipError::MissingInput {
                    required,
                    available: input_names,
                });
            }
        }
        let output_names = session
            .outputs
            .iter()
            .map(|output| output.name.clone())
            .collect::<Vec<_>>();
        for required in [TEXT_OUTPUT, IMAGE_OUTPUT] {
            if !output_names.iter().any(|name| name == required) {
                return Err(ClipError::MissingOutput {
                    required,
                    available: output_names,
                });
            }
        }

        let has_attention_mask = input_names.iter().any(|name| name == "attention_mask");
        Ok(Self {
            session,
            tokenizer,
            config,
            has_attention_mask,
        })
    }

    pub fn config(&self) -> &ClipConfig {
        &self.config
    }

    pub fn embed_text(&mut self, text: &str) -> Result<Vec<f32>, ClipError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(ClipError::EmptyText);
        }

        let encoding = self
            .tokenizer
            .encode(format!("{QUERY_PREFIX}{text}"), true)
            .map_err(|error| ClipError::Tokenizer(error.to_string()))?;
        let ids = encoding
            .get_ids()
            .iter()
            .map(|id| i64::from(*id))
            .collect::<Vec<_>>();
        let input_ids = Array2::from_shape_vec((1, ids.len()), ids)
            .expect("tokenizer returned a fixed one-row sequence");
        let attention_mask = Array2::from_elem((1, input_ids.ncols()), 1_i64);

        // The unified Jina ONNX graph requires a non-empty image batch on DirectML.
        // A zero-sized [0, 3, 512, 512] tensor works on CPU but fails in DML.
        let dummy_pixels = Array4::<f32>::zeros((1, 3, IMAGE_SIZE as usize, IMAGE_SIZE as usize));
        let input_ids = TensorRef::from_array_view(&input_ids)?;
        let attention_mask = TensorRef::from_array_view(&attention_mask)?;
        let pixel_values = TensorRef::from_array_view(&dummy_pixels)?;
        let outputs = if self.has_attention_mask {
            self.session.run(ort::inputs! {
                "input_ids" => input_ids,
                "attention_mask" => attention_mask,
                "pixel_values" => pixel_values,
            })?
        } else {
            self.session.run(ort::inputs! {
                "input_ids" => input_ids,
                "pixel_values" => pixel_values,
            })?
        };
        extract_embeddings(&outputs[TEXT_OUTPUT], 1, "text")
            .map(|mut embeddings| embeddings.remove(0))
    }

    pub fn embed_image(&mut self, path: impl AsRef<Path>) -> Result<Vec<f32>, ClipError> {
        self.embed_images(&[path.as_ref().to_path_buf()])
            .map(|mut embeddings| embeddings.remove(0))
    }

    pub fn embed_images(&mut self, paths: &[PathBuf]) -> Result<Vec<Vec<f32>>, ClipError> {
        if paths.is_empty() {
            return Err(ClipError::EmptyImageBatch);
        }

        let values = paths
            .iter()
            .map(|path| decode_image(path).map(preprocess_image))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let pixels = Array4::from_shape_vec(
            (paths.len(), 3, IMAGE_SIZE as usize, IMAGE_SIZE as usize),
            values,
        )
        .expect("preprocessed images have a fixed shape");
        let input_ids = Array2::from_elem((paths.len(), 1), 1_i64);
        let attention_mask = Array2::from_elem((paths.len(), 1), 1_i64);
        let input_ids = TensorRef::from_array_view(&input_ids)?;
        let attention_mask = TensorRef::from_array_view(&attention_mask)?;
        let pixel_values = TensorRef::from_array_view(&pixels)?;
        let outputs = if self.has_attention_mask {
            self.session.run(ort::inputs! {
                "input_ids" => input_ids,
                "attention_mask" => attention_mask,
                "pixel_values" => pixel_values,
            })?
        } else {
            self.session.run(ort::inputs! {
                "input_ids" => input_ids,
                "pixel_values" => pixel_values,
            })?
        };
        extract_embeddings(&outputs[IMAGE_OUTPUT], paths.len(), "image")
    }
}

fn require_file(path: &Path, error: impl FnOnce(PathBuf) -> ClipError) -> Result<(), ClipError> {
    if path.is_file() {
        Ok(())
    } else {
        Err(error(path.to_path_buf()))
    }
}

fn create_session(path: &Path, provider: ExecutionProvider) -> Result<Session, ClipError> {
    let builder = Session::builder()?.with_optimization_level(GraphOptimizationLevel::Level3)?;
    let builder = match provider {
        ExecutionProvider::Cpu => builder.with_parallel_execution(true)?,
        ExecutionProvider::Auto => {
            #[cfg(windows)]
            {
                builder
                    .with_parallel_execution(false)?
                    .with_memory_pattern(false)?
                    .with_execution_providers([DirectMLExecutionProvider::default()
                        .build()
                        .fail_silently()])?
            }
            #[cfg(target_os = "macos")]
            {
                builder
                    .with_parallel_execution(false)?
                    .with_execution_providers([CoreMLExecutionProvider::default()
                        .build()
                        .fail_silently()])?
            }
            #[cfg(not(any(windows, target_os = "macos")))]
            {
                builder.with_parallel_execution(true)?
            }
        }
        ExecutionProvider::DirectMl => {
            #[cfg(windows)]
            {
                builder
                    .with_parallel_execution(false)?
                    .with_memory_pattern(false)?
                    .with_execution_providers([DirectMLExecutionProvider::default()
                        .build()
                        .error_on_failure()])?
            }
            #[cfg(not(windows))]
            {
                return Err(ClipError::UnsupportedExecutionProvider("direct_ml"));
            }
        }
    };
    Ok(builder.commit_from_file(path)?)
}

fn decode_image(path: &Path) -> Result<DynamicImage, ClipError> {
    let bytes = std::fs::read(path).map_err(|source| ClipError::ReadImage {
        path: path.to_path_buf(),
        source,
    })?;
    let format = image::guess_format(&bytes).map_err(|source| ClipError::DecodeImage {
        path: path.to_path_buf(),
        source,
    })?;
    if format == ImageFormat::Gif {
        return Err(ClipError::GifNotSupported(path.to_path_buf()));
    }
    image::load_from_memory_with_format(&bytes, format).map_err(|source| ClipError::DecodeImage {
        path: path.to_path_buf(),
        source,
    })
}

fn preprocess_image(image: DynamicImage) -> Vec<f32> {
    let image = resize_shortest_and_center_crop(image.to_rgb8());
    let plane_size = (IMAGE_SIZE * IMAGE_SIZE) as usize;
    let mut values = vec![0.0; plane_size * 3];
    for (index, pixel) in image.pixels().enumerate() {
        for channel in 0..3 {
            values[channel * plane_size + index] =
                (f32::from(pixel[channel]) / 255.0 - IMAGE_MEAN[channel]) / IMAGE_STD[channel];
        }
    }
    values
}

fn resize_shortest_and_center_crop(image: RgbImage) -> RgbImage {
    let (width, height) = image.dimensions();
    let scale = IMAGE_SIZE as f64 / width.min(height) as f64;
    let resized_width = ((width as f64 * scale).round() as u32).max(IMAGE_SIZE);
    let resized_height = ((height as f64 * scale).round() as u32).max(IMAGE_SIZE);
    let resized = image::imageops::resize(
        &image,
        resized_width,
        resized_height,
        FilterType::CatmullRom,
    );
    let x = (resized_width - IMAGE_SIZE) / 2;
    let y = (resized_height - IMAGE_SIZE) / 2;
    image::imageops::crop_imm(&resized, x, y, IMAGE_SIZE, IMAGE_SIZE).to_image()
}

fn extract_embeddings(
    value: &ort::value::DynValue,
    batch_size: usize,
    kind: &'static str,
) -> Result<Vec<Vec<f32>>, ClipError> {
    let (_, values) = value.try_extract_tensor::<f32>()?;
    let expected = batch_size * EMBEDDING_DIMENSION;
    if values.len() != expected {
        return Err(ClipError::InvalidOutputShape {
            kind,
            actual: values.len(),
            expected,
        });
    }
    values
        .chunks_exact(EMBEDDING_DIMENSION)
        .map(|embedding| normalize_embedding(embedding, kind))
        .collect()
}

fn normalize_embedding(values: &[f32], kind: &'static str) -> Result<Vec<f32>, ClipError> {
    if values.iter().any(|value| !value.is_finite()) {
        return Err(ClipError::NonFiniteEmbedding { kind });
    }
    let norm = values.iter().map(|value| value * value).sum::<f32>().sqrt();
    if !norm.is_finite() || norm <= f32::EPSILON {
        return Err(ClipError::ZeroNormEmbedding { kind });
    }
    Ok(values.iter().map(|value| value / norm).collect())
}

#[cfg(test)]
mod tests {
    use image::{ImageBuffer, Rgb};

    use super::*;

    #[test]
    fn preprocessing_uses_rgb_clip_normalization() {
        let image = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(
            IMAGE_SIZE,
            IMAGE_SIZE,
            Rgb([255, 0, 0]),
        ));
        let values = preprocess_image(image);
        let plane = (IMAGE_SIZE * IMAGE_SIZE) as usize;

        assert_eq!(values.len(), plane * 3);
        assert!((values[0] - (1.0 - IMAGE_MEAN[0]) / IMAGE_STD[0]).abs() < 1e-5);
        assert!((values[plane] - (0.0 - IMAGE_MEAN[1]) / IMAGE_STD[1]).abs() < 1e-5);
        assert!((values[plane * 2] - (0.0 - IMAGE_MEAN[2]) / IMAGE_STD[2]).abs() < 1e-5);
    }

    #[test]
    fn shortest_resize_is_center_cropped() {
        let image = ImageBuffer::from_fn(1024, 512, |x, _| {
            if !(256..768).contains(&x) {
                Rgb([255, 0, 0])
            } else {
                Rgb([0, 255, 0])
            }
        });
        let cropped = resize_shortest_and_center_crop(image);

        assert_eq!(cropped.dimensions(), (IMAGE_SIZE, IMAGE_SIZE));
        assert_eq!(cropped.get_pixel(0, 0), &Rgb([0, 255, 0]));
        assert_eq!(cropped.get_pixel(IMAGE_SIZE - 1, 0), &Rgb([0, 255, 0]));
    }

    #[test]
    fn normalization_rejects_invalid_vectors() {
        assert!(matches!(
            normalize_embedding(&[f32::NAN], "test"),
            Err(ClipError::NonFiniteEmbedding { .. })
        ));
        assert!(matches!(
            normalize_embedding(&[0.0, 0.0], "test"),
            Err(ClipError::ZeroNormEmbedding { .. })
        ));
    }
}
