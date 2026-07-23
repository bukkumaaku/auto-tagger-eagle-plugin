//! Persistent cosine vector search backed by SQLite and sqlite-vec.

use std::{collections::HashSet, path::Path, sync::Once};

use rusqlite::{Connection, OptionalExtension, Transaction, ffi::sqlite3_auto_extension, params};
use sqlite_vec::sqlite3_vec_init;
use thiserror::Error;

static REGISTER_SQLITE_VEC: Once = Once::new();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Modality {
    Image,
    Text,
}

impl Modality {
    fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Text => "text",
        }
    }

    fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "image" => Ok(Self::Image),
            "text" => Ok(Self::Text),
            _ => Err(StoreError::InvalidModality(value.to_string())),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VectorRecord {
    pub namespace: String,
    pub item_id: String,
    pub modality: Modality,
    pub source_key: String,
    pub source_uri: Option<String>,
    pub content: Option<String>,
    pub updated_at: i64,
    pub embedding: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchResult {
    pub row_id: i64,
    pub namespace: String,
    pub item_id: String,
    pub modality: Modality,
    pub source_key: String,
    pub source_uri: Option<String>,
    pub content: Option<String>,
    pub updated_at: i64,
    pub distance: f64,
    pub similarity: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StoredItem {
    pub item_id: String,
    pub source_uri: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StoredEmbedding {
    pub source_uri: Option<String>,
    pub content: Option<String>,
    pub updated_at: i64,
    pub embedding: Vec<f32>,
}

/// Read-only counts used by the UI before an embedding session is loaded.
/// This intentionally avoids opening/initializing a full `VectorStore`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VectorStoreStatus {
    pub indexed_count: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ModelCleanupResult {
    pub model_keys: Vec<String>,
    pub removed_tables: u64,
    pub removed_vectors: u64,
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("model key cannot be empty")]
    EmptyModelKey,

    #[error("vector dimension must be greater than zero")]
    InvalidDimension,

    #[error("model `{model_key}` uses vector dimension {actual}, requested {requested}")]
    DimensionMismatch {
        model_key: String,
        actual: usize,
        requested: usize,
    },

    #[error("embedding has dimension {actual}, expected {expected}")]
    EmbeddingDimension { actual: usize, expected: usize },

    #[error("embedding contains non-finite values")]
    NonFiniteEmbedding,

    #[error("embedding has zero L2 norm")]
    ZeroNormEmbedding,

    #[error("stored embedding has {actual} bytes, expected {expected}")]
    InvalidStoredEmbeddingLength { actual: usize, expected: usize },

    #[error("namespace, item_id, and source_key cannot be empty")]
    EmptyKey,

    #[error("top_k must be greater than zero")]
    InvalidTopK,

    #[error("database returned unsupported modality `{0}`")]
    InvalidModality(String),

    #[error("failed to create vector database directory {path}: {source}")]
    CreateDirectory {
        path: std::path::PathBuf,
        source: std::io::Error,
    },

    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

pub struct VectorStore {
    connection: Connection,
    model_id: i64,
    model_key: String,
    vector_table: String,
    dimension: usize,
}

impl VectorStore {
    pub fn read_status(
        path: impl AsRef<Path>,
        model_key: impl Into<String>,
        namespace: &str,
    ) -> Result<Option<VectorStoreStatus>, StoreError> {
        let model_key = model_key.into().trim().to_string();
        if model_key.is_empty() {
            return Err(StoreError::EmptyModelKey);
        }
        let path = path.as_ref();
        if !path.exists() {
            return Ok(None);
        }
        let connection = Connection::open(path)?;
        let table_exists = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'vector_models')",
            [],
            |row| row.get::<_, i64>(0),
        )?;
        if table_exists == 0 {
            return Ok(None);
        }
        let Some(model_id) = connection
            .query_row(
                "SELECT id FROM vector_models WHERE model_key = ?1",
                [&model_key],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
        else {
            return Ok(None);
        };
        let indexed_count = connection.query_row(
            "SELECT COUNT(*) FROM model_vector_items
             WHERE model_id = ?1 AND namespace = ?2 AND modality = 'image'",
            params![model_id, namespace],
            |row| row.get(0),
        )?;
        Ok(Some(VectorStoreStatus { indexed_count }))
    }

    pub fn stored_dimension(
        path: impl AsRef<Path>,
        model_key: impl Into<String>,
    ) -> Result<Option<usize>, StoreError> {
        let model_key = model_key.into().trim().to_string();
        if model_key.is_empty() {
            return Err(StoreError::EmptyModelKey);
        }
        let path = path.as_ref();
        if !path.exists() {
            return Ok(None);
        }
        let connection = Connection::open(path)?;
        let table_exists = connection.query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM sqlite_master
                 WHERE type = 'table' AND name = 'vector_models'
             )",
            [],
            |row| row.get::<_, i64>(0),
        )?;
        if table_exists == 0 {
            return Ok(None);
        }
        Ok(connection
            .query_row(
                "SELECT dimension FROM vector_models WHERE model_key = ?1",
                [&model_key],
                |row| row.get::<_, usize>(0),
            )
            .optional()?)
    }

    pub fn merge_model(&mut self, source_model_key: &str) -> Result<u64, StoreError> {
        let source_model_key = source_model_key.trim();
        if source_model_key.is_empty() || source_model_key == self.model_key {
            return Ok(0);
        }
        let source = self
            .connection
            .query_row(
                "SELECT id, dimension FROM vector_models WHERE model_key = ?1",
                [source_model_key],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, usize>(1)?)),
            )
            .optional()?;
        let Some((source_id, source_dimension)) = source else {
            return Ok(0);
        };
        if source_dimension != self.dimension {
            return Err(StoreError::DimensionMismatch {
                model_key: source_model_key.to_string(),
                actual: source_dimension,
                requested: self.dimension,
            });
        }
        let has_pending = self.connection.query_row(
            "SELECT EXISTS(
                 SELECT 1
                 FROM model_vector_items source
                 LEFT JOIN model_vector_items target
                   ON target.model_id = ?2
                  AND target.namespace = source.namespace
                  AND target.item_id = source.item_id
                  AND target.modality = source.modality
                  AND target.source_key = source.source_key
                 WHERE source.model_id = ?1
                   AND (target.id IS NULL OR source.updated_at > target.updated_at)
             )",
            params![source_id, self.model_id],
            |row| row.get::<_, bool>(0),
        )?;
        if !has_pending {
            return Ok(0);
        }
        let source_table = vector_table_name(source_id);
        let rows = {
            let mut statement = self.connection.prepare(&format!(
                "SELECT i.namespace, i.item_id, i.modality, i.source_key,
                        i.source_uri, i.content, i.updated_at, v.embedding
                 FROM model_vector_items i
                 JOIN {source_table} v ON v.rowid = i.id
                 LEFT JOIN model_vector_items target
                   ON target.model_id = ?2
                  AND target.namespace = i.namespace
                  AND target.item_id = i.item_id
                  AND target.modality = i.modality
                  AND target.source_key = i.source_key
                 WHERE i.model_id = ?1
                   AND (target.id IS NULL OR i.updated_at > target.updated_at)"
            ))?;
            statement
                .query_map(params![source_id, self.model_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, Vec<u8>>(7)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?
        };
        let records = rows
            .into_iter()
            .map(
                |(
                    namespace,
                    item_id,
                    modality,
                    source_key,
                    source_uri,
                    content,
                    updated_at,
                    blob,
                )| {
                    Ok(VectorRecord {
                        namespace,
                        item_id,
                        modality: Modality::parse(&modality)?,
                        source_key,
                        source_uri,
                        content,
                        updated_at,
                        embedding: embedding_from_blob(&blob, self.dimension)?,
                    })
                },
            )
            .collect::<Result<Vec<_>, StoreError>>()?;
        let imported = records.len() as u64;
        self.upsert_many(&records)?;
        Ok(imported)
    }

    pub fn delete_compatible_models(&mut self) -> Result<ModelCleanupResult, StoreError> {
        let Some((prefix, _)) = self.model_key.rsplit_once(':') else {
            return Ok(ModelCleanupResult::default());
        };
        let family_prefix = format!("{prefix}:");
        let models = {
            let mut statement = self.connection.prepare(
                "SELECT id, model_key,
                        (SELECT COUNT(*) FROM model_vector_items WHERE model_id = vector_models.id)
                 FROM vector_models
                 WHERE dimension = ?1 AND model_key <> ?2",
            )?;
            statement
                .query_map(params![self.dimension, self.model_key], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, u64>(2)?,
                    ))
                })?
                .filter_map(Result::ok)
                .filter(|(_, key, _)| key.starts_with(&family_prefix))
                .collect::<Vec<_>>()
        };
        if models.is_empty() {
            return Ok(ModelCleanupResult::default());
        }
        let transaction = self.connection.transaction()?;
        for (model_id, _, _) in &models {
            transaction.execute(
                &format!("DROP TABLE IF EXISTS {}", vector_table_name(*model_id)),
                [],
            )?;
            transaction.execute("DELETE FROM vector_models WHERE id = ?1", [model_id])?;
        }
        transaction.commit()?;
        Ok(ModelCleanupResult {
            model_keys: models.iter().map(|(_, key, _)| key.clone()).collect(),
            removed_tables: models.len() as u64,
            removed_vectors: models.iter().map(|(_, _, count)| count).sum(),
        })
    }

    pub fn open(
        path: impl AsRef<Path>,
        model_key: impl Into<String>,
        dimension: usize,
    ) -> Result<Self, StoreError> {
        let model_key = validate_model(model_key.into(), dimension)?;
        register_sqlite_vec();
        let path = path.as_ref();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).map_err(|source| StoreError::CreateDirectory {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let connection = Connection::open(path)?;
        Self::initialize(connection, model_key, dimension)
    }

    pub fn open_in_memory(
        model_key: impl Into<String>,
        dimension: usize,
    ) -> Result<Self, StoreError> {
        let model_key = validate_model(model_key.into(), dimension)?;
        register_sqlite_vec();
        let connection = Connection::open_in_memory()?;
        Self::initialize(connection, model_key, dimension)
    }

    fn initialize(
        connection: Connection,
        model_key: String,
        dimension: usize,
    ) -> Result<Self, StoreError> {
        connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS vector_models (
                 id INTEGER PRIMARY KEY,
                 model_key TEXT NOT NULL UNIQUE,
                 dimension INTEGER NOT NULL CHECK (dimension > 0)
             );
             CREATE TABLE IF NOT EXISTS model_vector_items (
                 id INTEGER PRIMARY KEY,
                 model_id INTEGER NOT NULL REFERENCES vector_models(id) ON DELETE CASCADE,
                 namespace TEXT NOT NULL,
                 item_id TEXT NOT NULL,
                 modality TEXT NOT NULL CHECK (modality IN ('image', 'text')),
                 source_key TEXT NOT NULL,
                 source_uri TEXT,
                 content TEXT,
                 updated_at INTEGER NOT NULL,
                 UNIQUE(model_id, namespace, item_id, modality, source_key)
             );
             CREATE INDEX IF NOT EXISTS model_vector_items_namespace
                 ON model_vector_items(model_id, namespace, modality);",
        )?;

        connection.execute(
            "INSERT INTO vector_models(model_key, dimension) VALUES (?1, ?2)
             ON CONFLICT(model_key) DO NOTHING",
            params![model_key, dimension],
        )?;
        let (model_id, actual) = connection.query_row(
            "SELECT id, dimension FROM vector_models WHERE model_key = ?1",
            [&model_key],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, usize>(1)?)),
        )?;
        if actual != dimension {
            return Err(StoreError::DimensionMismatch {
                model_key,
                actual,
                requested: dimension,
            });
        }

        let vector_table = format!("model_vectors_{model_id}");
        let create_vectors = format!(
            "CREATE VIRTUAL TABLE IF NOT EXISTS {vector_table} USING vec0(
                embedding float[{dimension}] distance_metric=cosine,
                namespace text,
                modality text
            )"
        );
        connection.execute(&create_vectors, [])?;
        Ok(Self {
            connection,
            model_id,
            model_key,
            vector_table,
            dimension,
        })
    }

    pub fn model_key(&self) -> &str {
        &self.model_key
    }

    pub fn dimension(&self) -> usize {
        self.dimension
    }

    pub fn sqlite_vec_version(&self) -> Result<String, StoreError> {
        Ok(self
            .connection
            .query_row("SELECT vec_version()", [], |row| row.get(0))?)
    }

    pub fn upsert(&mut self, record: &VectorRecord) -> Result<i64, StoreError> {
        self.upsert_many(std::slice::from_ref(record))
            .map(|row_ids| row_ids[0])
    }

    pub fn upsert_many(&mut self, records: &[VectorRecord]) -> Result<Vec<i64>, StoreError> {
        for record in records {
            validate_record(record, self.dimension)?;
        }
        let transaction = self.connection.transaction()?;
        let row_ids = records
            .iter()
            .map(|record| upsert_record(&transaction, self.model_id, &self.vector_table, record))
            .collect::<Result<Vec<_>, _>>()?;
        transaction.commit()?;
        Ok(row_ids)
    }

    pub fn delete(
        &mut self,
        namespace: &str,
        item_id: &str,
        modality: Modality,
        source_key: &str,
    ) -> Result<bool, StoreError> {
        let transaction = self.connection.transaction()?;
        let row_id = transaction
            .query_row(
                "SELECT id FROM model_vector_items
                 WHERE model_id = ?1 AND namespace = ?2 AND item_id = ?3
                   AND modality = ?4 AND source_key = ?5",
                params![
                    self.model_id,
                    namespace,
                    item_id,
                    modality.as_str(),
                    source_key
                ],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        let Some(row_id) = row_id else {
            return Ok(false);
        };
        transaction.execute(
            &format!("DELETE FROM {} WHERE rowid = ?1", self.vector_table),
            [row_id],
        )?;
        transaction.execute(
            "DELETE FROM model_vector_items WHERE id = ?1 AND model_id = ?2",
            params![row_id, self.model_id],
        )?;
        transaction.commit()?;
        Ok(true)
    }

    pub fn delete_missing(
        &mut self,
        namespace: &str,
        modality: Modality,
        source_key: &str,
        keep_item_ids: &HashSet<String>,
    ) -> Result<u64, StoreError> {
        let transaction = self.connection.transaction()?;
        let stale_row_ids = {
            let mut statement = transaction.prepare(
                "SELECT id, item_id FROM model_vector_items
                 WHERE model_id = ?1 AND namespace = ?2
                   AND modality = ?3 AND source_key = ?4",
            )?;
            statement
                .query_map(
                    params![self.model_id, namespace, modality.as_str(), source_key],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
                )?
                .filter_map(|row| match row {
                    Ok((row_id, item_id)) if !keep_item_ids.contains(&item_id) => Some(Ok(row_id)),
                    Ok(_) => None,
                    Err(error) => Some(Err(error)),
                })
                .collect::<Result<Vec<_>, _>>()?
        };

        for row_id in &stale_row_ids {
            transaction.execute(
                &format!("DELETE FROM {} WHERE rowid = ?1", self.vector_table),
                [row_id],
            )?;
            transaction.execute(
                "DELETE FROM model_vector_items WHERE id = ?1 AND model_id = ?2",
                params![row_id, self.model_id],
            )?;
        }
        transaction.commit()?;
        Ok(stale_row_ids.len() as u64)
    }

    pub fn count(&self, namespace: &str) -> Result<u64, StoreError> {
        Ok(self.connection.query_row(
            "SELECT COUNT(*) FROM model_vector_items
             WHERE model_id = ?1 AND namespace = ?2",
            params![self.model_id, namespace],
            |row| row.get(0),
        )?)
    }

    pub fn count_modality(&self, namespace: &str, modality: Modality) -> Result<u64, StoreError> {
        Ok(self.connection.query_row(
            "SELECT COUNT(*) FROM model_vector_items
             WHERE model_id = ?1 AND namespace = ?2 AND modality = ?3",
            params![self.model_id, namespace, modality.as_str()],
            |row| row.get(0),
        )?)
    }

    pub fn list_items(
        &self,
        namespace: &str,
        modality: Modality,
        source_key: &str,
    ) -> Result<Vec<StoredItem>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT item_id, source_uri FROM model_vector_items
             WHERE model_id = ?1 AND namespace = ?2
               AND modality = ?3 AND source_key = ?4
             ORDER BY item_id",
        )?;
        Ok(statement
            .query_map(
                params![self.model_id, namespace, modality.as_str(), source_key],
                |row| {
                    Ok(StoredItem {
                        item_id: row.get(0)?,
                        source_uri: row.get(1)?,
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn item_updated_at(
        &self,
        namespace: &str,
        item_id: &str,
        modality: Modality,
        source_key: &str,
    ) -> Result<Option<i64>, StoreError> {
        Ok(self
            .connection
            .query_row(
                "SELECT updated_at FROM model_vector_items
                 WHERE model_id = ?1 AND namespace = ?2 AND item_id = ?3
                   AND modality = ?4 AND source_key = ?5",
                params![
                    self.model_id,
                    namespace,
                    item_id,
                    modality.as_str(),
                    source_key
                ],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn get_embedding(
        &self,
        namespace: &str,
        item_id: &str,
        modality: Modality,
        source_key: &str,
    ) -> Result<Option<StoredEmbedding>, StoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT i.source_uri, i.content, i.updated_at, v.embedding
             FROM model_vector_items i
             JOIN {} v ON v.rowid = i.id
             WHERE i.model_id = ?1 AND i.namespace = ?2 AND i.item_id = ?3
               AND i.modality = ?4 AND i.source_key = ?5",
            self.vector_table
        ))?;
        let stored = statement
            .query_row(
                params![
                    self.model_id,
                    namespace,
                    item_id,
                    modality.as_str(),
                    source_key
                ],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, Vec<u8>>(3)?,
                    ))
                },
            )
            .optional()?;
        stored
            .map(|(source_uri, content, updated_at, blob)| {
                Ok(StoredEmbedding {
                    source_uri,
                    content,
                    updated_at,
                    embedding: embedding_from_blob(&blob, self.dimension)?,
                })
            })
            .transpose()
    }

    pub fn search(
        &self,
        namespace: &str,
        modality: Option<Modality>,
        query: &[f32],
        top_k: usize,
    ) -> Result<Vec<SearchResult>, StoreError> {
        validate_embedding(query, self.dimension)?;
        if top_k == 0 {
            return Err(StoreError::InvalidTopK);
        }
        let query_blob = embedding_blob(query);
        let mut statement = if modality.is_some() {
            self.connection.prepare(&format!(
                "SELECT i.id, i.namespace, i.item_id, i.modality, i.source_key,
                        i.source_uri, i.content, i.updated_at, v.distance
                 FROM {} v
                 JOIN model_vector_items i ON i.id = v.rowid
                 WHERE v.embedding MATCH ?1 AND k = ?2
                   AND v.namespace = ?3 AND v.modality = ?4
                   AND i.model_id = ?5
                 ORDER BY v.distance",
                self.vector_table
            ))?
        } else {
            self.connection.prepare(&format!(
                "SELECT i.id, i.namespace, i.item_id, i.modality, i.source_key,
                        i.source_uri, i.content, i.updated_at, v.distance
                 FROM {} v
                 JOIN model_vector_items i ON i.id = v.rowid
                 WHERE v.embedding MATCH ?1 AND k = ?2 AND v.namespace = ?3
                   AND i.model_id = ?4
                 ORDER BY v.distance",
                self.vector_table
            ))?
        };

        let results = if let Some(modality) = modality {
            statement
                .query_map(
                    params![
                        query_blob,
                        top_k as i64,
                        namespace,
                        modality.as_str(),
                        self.model_id
                    ],
                    map_search_row,
                )?
                .collect::<Result<Vec<_>, _>>()?
        } else {
            statement
                .query_map(
                    params![query_blob, top_k as i64, namespace, self.model_id],
                    map_search_row,
                )?
                .collect::<Result<Vec<_>, _>>()?
        };
        Ok(results)
    }
}

fn map_search_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SearchResult> {
    let modality = row.get::<_, String>(3)?;
    let distance = row.get::<_, f64>(8)?;
    Ok(SearchResult {
        row_id: row.get(0)?,
        namespace: row.get(1)?,
        item_id: row.get(2)?,
        modality: Modality::parse(&modality).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                3,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        source_key: row.get(4)?,
        source_uri: row.get(5)?,
        content: row.get(6)?,
        updated_at: row.get(7)?,
        distance,
        similarity: 1.0 - distance,
    })
}

fn register_sqlite_vec() {
    REGISTER_SQLITE_VEC.call_once(|| unsafe {
        type ExtensionInit = unsafe extern "C" fn(
            *mut rusqlite::ffi::sqlite3,
            *mut *mut std::ffi::c_char,
            *const rusqlite::ffi::sqlite3_api_routines,
        ) -> std::ffi::c_int;
        let init = std::mem::transmute::<*const (), ExtensionInit>(sqlite3_vec_init as *const ());
        sqlite3_auto_extension(Some(init));
    });
}

fn validate_model(model_key: String, dimension: usize) -> Result<String, StoreError> {
    let model_key = model_key.trim().to_string();
    if model_key.is_empty() {
        return Err(StoreError::EmptyModelKey);
    }
    if dimension == 0 {
        return Err(StoreError::InvalidDimension);
    }
    Ok(model_key)
}

fn validate_record(record: &VectorRecord, dimension: usize) -> Result<(), StoreError> {
    if record.namespace.trim().is_empty()
        || record.item_id.trim().is_empty()
        || record.source_key.trim().is_empty()
    {
        return Err(StoreError::EmptyKey);
    }
    validate_embedding(&record.embedding, dimension)
}

fn validate_embedding(embedding: &[f32], dimension: usize) -> Result<(), StoreError> {
    if embedding.len() != dimension {
        return Err(StoreError::EmbeddingDimension {
            actual: embedding.len(),
            expected: dimension,
        });
    }
    if embedding.iter().any(|value| !value.is_finite()) {
        return Err(StoreError::NonFiniteEmbedding);
    }
    let norm_squared = embedding.iter().map(|value| value * value).sum::<f32>();
    if norm_squared <= f32::EPSILON {
        return Err(StoreError::ZeroNormEmbedding);
    }
    Ok(())
}

fn upsert_metadata(
    transaction: &Transaction<'_>,
    model_id: i64,
    record: &VectorRecord,
) -> Result<i64, StoreError> {
    transaction.execute(
        "INSERT INTO model_vector_items(
             model_id, namespace, item_id, modality, source_key,
             source_uri, content, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(model_id, namespace, item_id, modality, source_key) DO UPDATE SET
             source_uri = excluded.source_uri,
             content = excluded.content,
             updated_at = excluded.updated_at",
        params![
            model_id,
            record.namespace,
            record.item_id,
            record.modality.as_str(),
            record.source_key,
            record.source_uri,
            record.content,
            record.updated_at,
        ],
    )?;
    Ok(transaction.query_row(
        "SELECT id FROM model_vector_items
         WHERE model_id = ?1 AND namespace = ?2 AND item_id = ?3
           AND modality = ?4 AND source_key = ?5",
        params![
            model_id,
            record.namespace,
            record.item_id,
            record.modality.as_str(),
            record.source_key,
        ],
        |row| row.get(0),
    )?)
}

fn upsert_record(
    transaction: &Transaction<'_>,
    model_id: i64,
    vector_table: &str,
    record: &VectorRecord,
) -> Result<i64, StoreError> {
    let row_id = upsert_metadata(transaction, model_id, record)?;
    transaction.execute(
        &format!("DELETE FROM {vector_table} WHERE rowid = ?1"),
        [row_id],
    )?;
    transaction.execute(
        &format!(
            "INSERT INTO {vector_table}(rowid, embedding, namespace, modality)
             VALUES (?1, ?2, ?3, ?4)"
        ),
        params![
            row_id,
            embedding_blob(&record.embedding),
            record.namespace,
            record.modality.as_str(),
        ],
    )?;
    Ok(row_id)
}

fn embedding_blob(embedding: &[f32]) -> Vec<u8> {
    embedding
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn vector_table_name(model_id: i64) -> String {
    format!("model_vectors_{model_id}")
}

fn embedding_from_blob(blob: &[u8], dimension: usize) -> Result<Vec<f32>, StoreError> {
    let expected = dimension * std::mem::size_of::<f32>();
    if blob.len() != expected {
        return Err(StoreError::InvalidStoredEmbeddingLength {
            actual: blob.len(),
            expected,
        });
    }
    Ok(blob
        .chunks_exact(std::mem::size_of::<f32>())
        .map(|bytes| f32::from_le_bytes(bytes.try_into().expect("four-byte f32 chunk")))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_MODEL: &str = "jina-clip-v2-q8";

    fn memory_store(dimension: usize) -> VectorStore {
        VectorStore::open_in_memory(TEST_MODEL, dimension).unwrap()
    }

    fn record(item_id: &str, modality: Modality, embedding: Vec<f32>) -> VectorRecord {
        VectorRecord {
            namespace: "library-a".to_string(),
            item_id: item_id.to_string(),
            modality,
            source_key: "primary".to_string(),
            source_uri: Some(format!("{item_id}.png")),
            content: None,
            updated_at: 1,
            embedding,
        }
    }

    #[test]
    fn sqlite_vec_knn_respects_namespace_and_modality() {
        let mut store = memory_store(4);
        store
            .upsert(&record("red", Modality::Image, vec![1.0, 0.0, 0.0, 0.0]))
            .unwrap();
        store
            .upsert(&record("blue", Modality::Image, vec![0.0, 1.0, 0.0, 0.0]))
            .unwrap();
        let mut other = record("other", Modality::Image, vec![1.0, 0.0, 0.0, 0.0]);
        other.namespace = "library-b".to_string();
        store.upsert(&other).unwrap();
        store
            .upsert(&record("text", Modality::Text, vec![1.0, 0.0, 0.0, 0.0]))
            .unwrap();

        let results = store
            .search(
                "library-a",
                Some(Modality::Image),
                &[1.0, 0.0, 0.0, 0.0],
                10,
            )
            .unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].item_id, "red");
        assert!(results[0].similarity > 0.999);
        assert!(results.iter().all(|item| item.namespace == "library-a"));
        assert!(results.iter().all(|item| item.modality == Modality::Image));
        assert!(store.sqlite_vec_version().unwrap().starts_with('v'));
    }

    #[test]
    fn upsert_replaces_vector_without_changing_identity() {
        let mut store = memory_store(2);
        let first_id = store
            .upsert(&record("item", Modality::Image, vec![1.0, 0.0]))
            .unwrap();
        let mut replacement = record("item", Modality::Image, vec![0.0, 1.0]);
        replacement.updated_at = 2;
        let second_id = store.upsert(&replacement).unwrap();

        assert_eq!(first_id, second_id);
        assert_eq!(store.count("library-a").unwrap(), 1);
        let results = store.search("library-a", None, &[0.0, 1.0], 1).unwrap();
        assert_eq!(results[0].item_id, "item");
        assert!(results[0].similarity > 0.999);
        assert_eq!(
            store
                .item_updated_at("library-a", "item", Modality::Image, "primary")
                .unwrap(),
            Some(2)
        );
        assert_eq!(
            store.count_modality("library-a", Modality::Image).unwrap(),
            1
        );
        assert_eq!(
            store.count_modality("library-a", Modality::Text).unwrap(),
            0
        );
    }

    #[test]
    fn retrieves_embedding_by_full_identity() {
        let mut store = memory_store(2);
        let mut stored = record("item", Modality::Image, vec![0.25, 0.75]);
        stored.content = Some("sample".to_string());
        stored.updated_at = 42;
        store.upsert(&stored).unwrap();

        let embedding = store
            .get_embedding("library-a", "item", Modality::Image, "primary")
            .unwrap()
            .unwrap();
        assert_eq!(embedding.source_uri.as_deref(), Some("item.png"));
        assert_eq!(embedding.content.as_deref(), Some("sample"));
        assert_eq!(embedding.updated_at, 42);
        assert_eq!(embedding.embedding, vec![0.25, 0.75]);
        assert!(
            store
                .get_embedding("library-a", "missing", Modality::Image, "primary")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn batch_upsert_uses_one_consistent_transaction() {
        let mut store = memory_store(2);
        let row_ids = store
            .upsert_many(&[
                record("first", Modality::Image, vec![1.0, 0.0]),
                record("second", Modality::Text, vec![0.0, 1.0]),
            ])
            .unwrap();

        assert_eq!(row_ids.len(), 2);
        assert_eq!(store.count("library-a").unwrap(), 2);
    }

    #[test]
    fn delete_removes_metadata_and_vector() {
        let mut store = memory_store(2);
        store
            .upsert(&record("item", Modality::Image, vec![1.0, 0.0]))
            .unwrap();

        assert!(
            store
                .delete("library-a", "item", Modality::Image, "primary")
                .unwrap()
        );
        assert_eq!(store.count("library-a").unwrap(), 0);
        assert!(
            store
                .search("library-a", None, &[1.0, 0.0], 1)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn delete_missing_removes_only_stale_items() {
        let mut store = memory_store(2);
        store
            .upsert_many(&[
                record("keep", Modality::Image, vec![1.0, 0.0]),
                record("remove", Modality::Image, vec![0.0, 1.0]),
                record("text", Modality::Text, vec![1.0, 0.0]),
            ])
            .unwrap();

        let removed = store
            .delete_missing(
                "library-a",
                Modality::Image,
                "primary",
                &HashSet::from(["keep".to_string()]),
            )
            .unwrap();

        assert_eq!(removed, 1);
        assert_eq!(
            store.count_modality("library-a", Modality::Image).unwrap(),
            1
        );
        assert_eq!(
            store.count_modality("library-a", Modality::Text).unwrap(),
            1
        );
        assert_eq!(
            store
                .search("library-a", Some(Modality::Image), &[1.0, 0.0], 10,)
                .unwrap()[0]
                .item_id,
            "keep"
        );
    }

    #[test]
    fn lists_image_metadata_for_health_checks() {
        let mut store = memory_store(2);
        store
            .upsert_many(&[
                record("second", Modality::Image, vec![1.0, 0.0]),
                record("first", Modality::Image, vec![0.0, 1.0]),
                record("text", Modality::Text, vec![1.0, 0.0]),
            ])
            .unwrap();

        let items = store
            .list_items("library-a", Modality::Image, "primary")
            .unwrap();

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].item_id, "first");
        assert_eq!(items[0].source_uri.as_deref(), Some("first.png"));
        assert_eq!(items[1].item_id, "second");
    }

    #[test]
    fn rejects_dimension_mismatch_and_invalid_values() {
        let mut store = memory_store(2);
        assert!(matches!(
            VectorStore::open_in_memory(" ", 2),
            Err(StoreError::EmptyModelKey)
        ));
        assert!(matches!(
            store.upsert(&record("bad", Modality::Image, vec![1.0])),
            Err(StoreError::EmbeddingDimension { .. })
        ));
        assert!(matches!(
            store.upsert(&record("bad", Modality::Image, vec![f32::NAN, 0.0])),
            Err(StoreError::NonFiniteEmbedding)
        ));
        assert!(matches!(
            store.upsert(&record("bad", Modality::Image, vec![0.0, 0.0])),
            Err(StoreError::ZeroNormEmbedding)
        ));
    }

    #[test]
    fn different_models_use_isolated_vector_tables() {
        let path = temporary_database_path("models");
        {
            let mut first = VectorStore::open(&path, "jina-clip-v2-q8", 2).unwrap();
            first
                .upsert(&record("shared-item", Modality::Image, vec![1.0, 0.0]))
                .unwrap();
        }
        {
            let mut second = VectorStore::open(&path, "other-clip-768", 3).unwrap();
            second
                .upsert(&record("shared-item", Modality::Image, vec![0.0, 1.0, 0.0]))
                .unwrap();

            assert_eq!(second.model_key(), "other-clip-768");
            assert_eq!(second.count("library-a").unwrap(), 1);
            assert_eq!(
                second
                    .search("library-a", None, &[0.0, 1.0, 0.0], 1)
                    .unwrap()[0]
                    .item_id,
                "shared-item"
            );
        }
        {
            let first = VectorStore::open(&path, "jina-clip-v2-q8", 2).unwrap();
            assert_eq!(first.count("library-a").unwrap(), 1);
            assert!(matches!(
                VectorStore::open(&path, "jina-clip-v2-q8", 3),
                Err(StoreError::DimensionMismatch { .. })
            ));

            let table_count = first
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master
                     WHERE type = 'table'
                       AND name GLOB 'model_vectors_[0-9]*'
                       AND sql LIKE '%USING vec0%'",
                    [],
                    |row| row.get::<_, u64>(0),
                )
                .unwrap();
            assert_eq!(table_count, 2);
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn merges_vectors_from_a_legacy_model_key() {
        let path = temporary_database_path("import");
        {
            let mut legacy = VectorStore::open(&path, "legacy", 2).unwrap();
            legacy
                .upsert(&record("legacy-item", Modality::Image, vec![1.0, 0.0]))
                .unwrap();
            legacy
                .upsert(&record("shared-item", Modality::Image, vec![1.0, 0.0]))
                .unwrap();
        }
        let mut current = VectorStore::open(&path, "current", 2).unwrap();
        let mut current_record = record("shared-item", Modality::Image, vec![0.0, 1.0]);
        current_record.updated_at = 2;
        current.upsert(&current_record).unwrap();
        assert_eq!(current.merge_model("legacy").unwrap(), 1);
        assert_eq!(current.merge_model("legacy").unwrap(), 0);
        assert_eq!(current.count("library-a").unwrap(), 2);
        assert_eq!(
            current
                .search("library-a", Some(Modality::Image), &[1.0, 0.0], 1)
                .unwrap()[0]
                .item_id,
            "legacy-item"
        );
        assert_eq!(
            current
                .get_embedding("library-a", "shared-item", Modality::Image, "primary")
                .unwrap()
                .unwrap()
                .embedding,
            vec![0.0, 1.0]
        );
        drop(current);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn deletes_vectors_from_compatible_endpoint_specific_model_keys() {
        let path = temporary_database_path("compatible-import");
        let legacy_key = "gemini:gemini-embedding-2-preview:3072:old-endpoint";
        let current_key = "gemini:gemini-embedding-2-preview:3072:new-endpoint";
        {
            let mut legacy = VectorStore::open(&path, legacy_key, 2).unwrap();
            legacy
                .upsert(&record("legacy-item", Modality::Image, vec![1.0, 0.0]))
                .unwrap();
        }
        let mut current = VectorStore::open(&path, current_key, 2).unwrap();
        let result = current.delete_compatible_models().unwrap();
        assert_eq!(result.model_keys, vec![legacy_key]);
        assert_eq!(result.removed_tables, 1);
        assert_eq!(result.removed_vectors, 1);
        assert_eq!(current.count("library-a").unwrap(), 0);
        assert_eq!(
            VectorStore::stored_dimension(&path, legacy_key).unwrap(),
            None
        );
        drop(current);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn disk_store_survives_reopen() {
        let path = temporary_database_path("reopen");
        {
            let mut store = VectorStore::open(&path, TEST_MODEL, 2).unwrap();
            store
                .upsert(&record("persisted", Modality::Image, vec![1.0, 0.0]))
                .unwrap();
        }
        {
            let store = VectorStore::open(&path, TEST_MODEL, 2).unwrap();
            let results = store
                .search("library-a", Some(Modality::Image), &[1.0, 0.0], 1)
                .unwrap();
            assert_eq!(results[0].item_id, "persisted");
        }
        std::fs::remove_file(path).unwrap();
    }

    fn temporary_database_path(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "eagle-vector-store-{label}-{}-{}.sqlite3",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
}
