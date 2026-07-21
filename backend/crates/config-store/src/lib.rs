use std::{
    env, fs, io,
    path::{Path, PathBuf},
    sync::Arc,
};

#[cfg(test)]
use std::{collections::HashMap, sync::Mutex};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const CONFIG_DIRECTORY: &str = ".eagle-auto-tagger";
pub const CONFIG_FILENAME: &str = "config.json5";
pub const DEFAULT_LLM_TAG_PROMPT: &str = "给这张图打标签，标签内容包括主体、人物、表情、动作、物品、场景、风格、视角和拍摄角度。请输出10个不重复的简短中文标签，使用逗号分隔，仅回复标签。";
pub const DEFAULT_LLM_ANNOTATION_PROMPT: &str =
    "请用一句简洁自然的中文描述这张图片的主体、场景、风格和关键细节，仅回复描述内容。";
const KEYRING_SERVICE: &str = "eagle-auto-tagger";
const SECRET_REFERENCE_PREFIX: &str = "$keyring:v1:";
const ENDPOINT_FIELD: &str = "endpoint";
const API_KEY_FIELD: &str = "apiKey";
const ENDPOINT_SECRET: &str = "config.endpoint";
const API_KEY_SECRET: &str = "config.api-key";
const LLM_ENDPOINT_FIELD: &str = "llmEndpoint";
const LLM_API_KEY_FIELD: &str = "llmApiKey";
const LLM_ENDPOINT_SECRET: &str = "config.llm-endpoint";
const LLM_API_KEY_SECRET: &str = "config.llm-api-key";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RemoteEmbeddingProfile {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub endpoint: String,
    pub api_key: String,
    pub model: String,
    pub dimension: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RemoteLlmProfile {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub endpoint: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    pub endpoint: String,
    pub api_key: String,
    pub llm_provider: String,
    pub llm_endpoint: String,
    pub llm_api_key: String,
    pub llm_remote_model: String,
    pub llm_remote_concurrency: u64,
    pub embedding_provider: String,
    pub embedding_dimension: u64,
    pub embedding_remote_profiles: Vec<RemoteEmbeddingProfile>,
    pub embedding_remote_profile_id: String,
    pub llm_remote_profiles: Vec<RemoteLlmProfile>,
    pub llm_remote_profile_id: String,
    pub model_path: String,
    pub threshold: f64,
    pub steps: u64,
    pub filter_tags: Vec<String>,
    pub overwrite: String,
    pub language: String,
    pub splitter: String,
    pub read_video: String,
    pub model_location: String,
    pub llm_model_path: String,
    pub llm_runner_path: String,
    pub llm_context_size: u64,
    #[serde(rename = "llmNGL")]
    pub llm_ngl: String,
    pub llm_gpu: String,
    pub llm_use_vulkan: bool,
    pub llm_temperature: f64,
    pub llm_max_tokens: u64,
    pub llm_overwrite: String,
    pub llm_annotation_overwrite: String,
    pub llm_tagger_or_annotation: String,
    pub llm_tagger_prompt: String,
    pub llm_annotation_prompt: String,
    pub embedding_model_id: String,
    pub embedding_model_name: String,
    pub embedding_device: String,
    pub embedding_batch_size: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            api_key: String::new(),
            llm_provider: "local".to_string(),
            llm_endpoint: String::new(),
            llm_api_key: String::new(),
            llm_remote_model: String::new(),
            llm_remote_concurrency: 4,
            embedding_provider: "open_ai".to_string(),
            embedding_dimension: 1_536,
            embedding_remote_profiles: Vec::new(),
            embedding_remote_profile_id: String::new(),
            llm_remote_profiles: Vec::new(),
            llm_remote_profile_id: String::new(),
            model_path: String::new(),
            threshold: 0.25,
            steps: 1,
            filter_tags: Vec::new(),
            overwrite: "merge".to_string(),
            language: "zh".to_string(),
            splitter: "|".to_string(),
            read_video: "noread".to_string(),
            model_location: String::new(),
            llm_model_path: "Qwen3VL-8B-Instruct-Q4_K_M".to_string(),
            llm_runner_path: String::new(),
            llm_context_size: 8192,
            llm_ngl: "9999".to_string(),
            llm_gpu: "nvidia".to_string(),
            llm_use_vulkan: false,
            llm_temperature: 0.5,
            llm_max_tokens: 1024,
            llm_overwrite: "merge".to_string(),
            llm_annotation_overwrite: "cover".to_string(),
            llm_tagger_or_annotation: "tagger".to_string(),
            llm_tagger_prompt: DEFAULT_LLM_TAG_PROMPT.to_string(),
            llm_annotation_prompt: DEFAULT_LLM_ANNOTATION_PROMPT.to_string(),
            embedding_model_id: "jina-clip-v2-q8".to_string(),
            embedding_model_name: String::new(),
            embedding_device: "auto".to_string(),
            embedding_batch_size: 8,
        }
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("could not determine the current user's home directory")]
    HomeDirectoryNotFound,

    #[error("failed to read config at {path}: {source}")]
    Read { path: PathBuf, source: io::Error },

    #[error("failed to parse JSON5 config at {path}: {source}")]
    Parse { path: PathBuf, source: json5::Error },

    #[error("failed to serialize config: {0}")]
    Serialize(#[from] serde_json::Error),

    #[error("failed to deserialize config at {path}: {source}")]
    Deserialize {
        path: PathBuf,
        source: serde_json::Error,
    },

    #[error("failed to create config directory {path}: {source}")]
    CreateDirectory { path: PathBuf, source: io::Error },

    #[error("failed to write config at {path}: {source}")]
    Write { path: PathBuf, source: io::Error },

    #[error("failed to {operation} protected config value `{key}`: {message}")]
    SecretStorage {
        operation: &'static str,
        key: &'static str,
        message: String,
    },

    #[error("protected config value `{key}` is invalid: {message}")]
    SecretInvalid { key: &'static str, message: String },
}

#[derive(Clone, Debug)]
pub struct ConfigStore {
    path: PathBuf,
    secrets: Arc<dyn SecretStore>,
}

impl ConfigStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            secrets: Arc::new(KeyringSecretStore),
        }
    }

    pub fn for_current_user() -> Result<Self, ConfigError> {
        Ok(Self::new(default_config_path()?))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn read(&self) -> Result<Config, ConfigError> {
        match fs::read_to_string(&self.path) {
            Ok(source) => {
                let stored: serde_json::Value =
                    json5::from_str(&source).map_err(|source| ConfigError::Parse {
                        path: self.path.clone(),
                        source,
                    })?;
                let mut plain = stored.clone();
                let obsolete_fields_removed = remove_obsolete_fields(&mut plain);
                let secrets_need_migration = self.resolve_sensitive_fields(&mut plain)?;
                let config: Config = serde_json::from_value(plain.clone()).map_err(|source| {
                    ConfigError::Deserialize {
                        path: self.path.clone(),
                        source,
                    }
                })?;
                let complete = serde_json::to_value(&config)?;

                if merge_missing_fields(&mut plain, &complete)
                    || secrets_need_migration
                    || obsolete_fields_removed
                {
                    self.protect_sensitive_fields(&mut plain)?;
                    self.write_value(&plain)?;
                }

                Ok(config)
            }
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                let config = Config::default();
                self.write(&config)?;
                Ok(config)
            }
            Err(source) => Err(ConfigError::Read {
                path: self.path.clone(),
                source,
            }),
        }
    }

    pub fn write(&self, config: &Config) -> Result<(), ConfigError> {
        let mut value = serde_json::to_value(config)?;
        self.protect_sensitive_fields(&mut value)?;
        self.write_value(&value)
    }

    fn resolve_sensitive_fields(&self, value: &mut serde_json::Value) -> Result<bool, ConfigError> {
        let Some(object) = value.as_object_mut() else {
            return Ok(false);
        };

        let mut needs_migration = false;
        for (field, secret_name) in sensitive_fields() {
            let Some(stored_value) = object.get_mut(field) else {
                continue;
            };
            let Some(stored_value) = stored_value.as_str() else {
                continue;
            };
            let reference = secret_reference(secret_name);
            if stored_value == reference {
                match self.secrets.get(secret_name)? {
                    Some(secret) => {
                        object.insert(field.to_string(), serde_json::Value::String(secret));
                    }
                    None => {
                        object.insert(field.to_string(), serde_json::Value::String(String::new()));
                        needs_migration = true;
                    }
                }
            } else if stored_value.starts_with(SECRET_REFERENCE_PREFIX) {
                return Err(ConfigError::SecretInvalid {
                    key: secret_name,
                    message: format!("unexpected secret reference `{stored_value}`"),
                });
            } else {
                needs_migration = true;
            }
        }
        Ok(needs_migration)
    }

    fn protect_sensitive_fields(&self, value: &mut serde_json::Value) -> Result<(), ConfigError> {
        let Some(object) = value.as_object_mut() else {
            return Ok(());
        };

        for (field, secret_name) in sensitive_fields() {
            let Some(value) = object.get(field).and_then(serde_json::Value::as_str) else {
                continue;
            };
            self.secrets.set(secret_name, value)?;
            object.insert(
                field.to_string(),
                serde_json::Value::String(secret_reference(secret_name)),
            );
        }
        Ok(())
    }

    fn write_value(&self, value: &serde_json::Value) -> Result<(), ConfigError> {
        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).map_err(|source| ConfigError::CreateDirectory {
            path: parent.to_path_buf(),
            source,
        })?;

        let mut source = serde_json::to_string_pretty(value)?;
        source.push('\n');
        fs::write(&self.path, source).map_err(|source| ConfigError::Write {
            path: self.path.clone(),
            source,
        })
    }
}

fn sensitive_fields() -> [(&'static str, &'static str); 4] {
    [
        (ENDPOINT_FIELD, ENDPOINT_SECRET),
        (API_KEY_FIELD, API_KEY_SECRET),
        (LLM_ENDPOINT_FIELD, LLM_ENDPOINT_SECRET),
        (LLM_API_KEY_FIELD, LLM_API_KEY_SECRET),
    ]
}

fn remove_obsolete_fields(value: &mut serde_json::Value) -> bool {
    let Some(object) = value.as_object_mut() else {
        return false;
    };
    let mut removed = false;
    for field in [
        "modelPathTagger",
        "modelPathJoy",
        "autotagger",
        "joy_type",
        "joy_length",
        "joy_extra",
        "activeWorker",
        "llmModelFile",
        "llmMmprojPath",
        "llmHost",
        "llmPort",
        "llmKeepServerAlive",
        "llmLanguage",
        "llmOfflineOrOnline",
        "embeddingModelPath",
        "embeddingTextDevice",
        "embeddingDtype",
        "embeddingTopK",
    ] {
        removed |= object.remove(field).is_some();
    }
    removed
}

fn secret_reference(secret_name: &str) -> String {
    format!("{SECRET_REFERENCE_PREFIX}{secret_name}")
}

trait SecretStore: Send + Sync + std::fmt::Debug {
    fn get(&self, key: &'static str) -> Result<Option<String>, ConfigError>;
    fn set(&self, key: &'static str, value: &str) -> Result<(), ConfigError>;
}

#[derive(Debug)]
struct KeyringSecretStore;

impl SecretStore for KeyringSecretStore {
    fn get(&self, key: &'static str) -> Result<Option<String>, ConfigError> {
        let entry = keyring_entry(key, "read")?;
        let encoded = match entry.get_password() {
            Ok(value) => value,
            Err(keyring::Error::NoEntry) => return Ok(None),
            Err(error) => return Err(secret_storage_error("read", key, error)),
        };
        let encoded = encoded
            .strip_prefix("v1:")
            .ok_or_else(|| ConfigError::SecretInvalid {
                key,
                message: "unsupported credential format".to_string(),
            })?;
        let bytes = BASE64
            .decode(encoded)
            .map_err(|error| ConfigError::SecretInvalid {
                key,
                message: error.to_string(),
            })?;
        String::from_utf8(bytes)
            .map(Some)
            .map_err(|error| ConfigError::SecretInvalid {
                key,
                message: error.to_string(),
            })
    }

    fn set(&self, key: &'static str, value: &str) -> Result<(), ConfigError> {
        let entry = keyring_entry(key, "write")?;
        let encoded = format!("v1:{}", BASE64.encode(value.as_bytes()));
        entry
            .set_password(&encoded)
            .map_err(|error| secret_storage_error("write", key, error))
    }
}

fn keyring_entry(
    key: &'static str,
    operation: &'static str,
) -> Result<keyring::Entry, ConfigError> {
    keyring::Entry::new(KEYRING_SERVICE, key)
        .map_err(|error| secret_storage_error(operation, key, error))
}

fn secret_storage_error(
    operation: &'static str,
    key: &'static str,
    error: impl std::fmt::Display,
) -> ConfigError {
    ConfigError::SecretStorage {
        operation,
        key,
        message: error.to_string(),
    }
}

#[cfg(test)]
#[derive(Debug, Default)]
struct MemorySecretStore {
    values: Mutex<HashMap<&'static str, String>>,
}

#[cfg(test)]
impl SecretStore for MemorySecretStore {
    fn get(&self, key: &'static str) -> Result<Option<String>, ConfigError> {
        Ok(self.values.lock().unwrap().get(key).cloned())
    }

    fn set(&self, key: &'static str, value: &str) -> Result<(), ConfigError> {
        self.values.lock().unwrap().insert(key, value.to_string());
        Ok(())
    }
}

fn merge_missing_fields(stored: &mut serde_json::Value, complete: &serde_json::Value) -> bool {
    let (Some(stored), Some(complete)) = (stored.as_object_mut(), complete.as_object()) else {
        return false;
    };

    let mut changed = false;
    for (key, default_value) in complete {
        if let Some(stored_value) = stored.get_mut(key) {
            changed |= merge_missing_fields(stored_value, default_value);
        } else {
            stored.insert(key.clone(), default_value.clone());
            changed = true;
        }
    }
    changed
}

pub fn default_config_path() -> Result<PathBuf, ConfigError> {
    home_directory()
        .map(|home| home.join(CONFIG_DIRECTORY).join(CONFIG_FILENAME))
        .ok_or(ConfigError::HomeDirectoryNotFound)
}

fn home_directory() -> Option<PathBuf> {
    env::var_os("USERPROFILE")
        .filter(|value| !value.is_empty())
        .or_else(|| env::var_os("HOME").filter(|value| !value.is_empty()))
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use std::{
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    fn test_store(name: &str) -> ConfigStore {
        let unique = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        ConfigStore {
            path: env::temp_dir().join(format!(
                "eagle-config-{name}-{}-{nanos}-{unique}.json5",
                std::process::id()
            )),
            secrets: Arc::new(MemorySecretStore::default()),
        }
    }

    #[test]
    fn creates_default_config_when_file_is_missing() {
        let store = test_store("default");

        let config = store.read().unwrap();

        assert_eq!(config, Config::default());
        assert_eq!(config.threshold, 0.25);
        assert_eq!(config.steps, 1);
        assert_eq!(config.language, "zh");
        assert_eq!(config.read_video, "noread");
        assert_eq!(config.overwrite, "merge");
        assert_eq!(config.splitter, "|");
        assert_eq!(config.llm_tagger_prompt, DEFAULT_LLM_TAG_PROMPT);
        assert_eq!(config.llm_annotation_prompt, DEFAULT_LLM_ANNOTATION_PROMPT);
        assert_eq!(config.llm_remote_concurrency, 4);
        assert_eq!(config.embedding_model_id, "jina-clip-v2-q8");
        assert!(config.embedding_model_name.is_empty());
        assert_eq!(config.embedding_batch_size, 8);
        assert!(store.path().is_file());
        let stored = fs::read_to_string(store.path()).unwrap();
        assert!(stored.contains("$keyring:v1:config.endpoint"));
        assert!(stored.contains("$keyring:v1:config.api-key"));
        fs::remove_file(store.path()).unwrap();
    }

    #[test]
    fn reads_json5_and_fills_missing_keys_with_defaults() {
        let store = test_store("json5");
        fs::write(
            store.path(),
            "{ // JSON5 comments are allowed\n modelPath: 'model.onnx',\n autotagger: true,\n embeddingTopK: 500,\n customKey: 'kept',\n }",
        )
        .unwrap();

        let config = store.read().unwrap();

        assert_eq!(config.model_path, "model.onnx");
        assert_eq!(config.threshold, 0.25);
        let migrated: serde_json::Value =
            json5::from_str(&fs::read_to_string(store.path()).unwrap()).unwrap();
        assert_eq!(migrated["modelPath"], "model.onnx");
        assert_eq!(migrated["threshold"], 0.25);
        assert_eq!(migrated["llmTaggerPrompt"], DEFAULT_LLM_TAG_PROMPT);
        assert_eq!(migrated["llmRemoteConcurrency"], 4);
        assert_eq!(migrated["endpoint"], "$keyring:v1:config.endpoint");
        assert_eq!(migrated["apiKey"], "$keyring:v1:config.api-key");
        assert_eq!(migrated["customKey"], "kept");
        assert!(migrated.get("autotagger").is_none());
        assert!(migrated.get("embeddingTopK").is_none());
        fs::remove_file(store.path()).unwrap();
    }

    #[test]
    fn writes_and_reads_config() {
        let store = test_store("roundtrip");
        let expected = Config {
            endpoint: "https://example.com/v1/embeddings".to_string(),
            api_key: "secret-api-key".to_string(),
            language: "zh".to_string(),
            ..Config::default()
        };

        store.write(&expected).unwrap();
        let source_before_read = fs::read_to_string(store.path()).unwrap();
        let actual = store.read().unwrap();

        assert_eq!(actual, expected);
        assert!(!source_before_read.contains("https://example.com/v1/embeddings"));
        assert!(!source_before_read.contains("secret-api-key"));
        assert_eq!(
            fs::read_to_string(store.path()).unwrap(),
            source_before_read
        );
        fs::remove_file(store.path()).unwrap();
    }

    #[test]
    fn migrates_plaintext_endpoint_and_api_key_to_protected_storage() {
        let store = test_store("plaintext-migration");
        fs::write(
            store.path(),
            "{ endpoint: 'https://example.com/v1/embeddings', apiKey: 'legacy-key' }",
        )
        .unwrap();

        let config = store.read().unwrap();
        let stored = fs::read_to_string(store.path()).unwrap();

        assert_eq!(config.endpoint, "https://example.com/v1/embeddings");
        assert_eq!(config.api_key, "legacy-key");
        assert!(!stored.contains("https://example.com/v1/embeddings"));
        assert!(!stored.contains("legacy-key"));
        assert!(stored.contains("$keyring:v1:config.endpoint"));
        assert!(stored.contains("$keyring:v1:config.api-key"));
        fs::remove_file(store.path()).unwrap();
    }

    #[test]
    fn missing_credentials_fall_back_to_empty_values() {
        let store = test_store("missing-credentials");
        fs::write(
            store.path(),
            "{ endpoint: '$keyring:v1:config.endpoint', apiKey: '$keyring:v1:config.api-key' }",
        )
        .unwrap();

        let config = store.read().unwrap();

        assert!(config.endpoint.is_empty());
        assert!(config.api_key.is_empty());
        fs::remove_file(store.path()).unwrap();
    }
}
