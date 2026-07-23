//! Model-aware text vector storage backed by SQLite and sqlite-vec.

use std::{collections::HashSet, path::Path, sync::Once, time::Duration};

use rusqlite::{Connection, OptionalExtension, Transaction, ffi::sqlite3_auto_extension, params};
use sqlite_vec::sqlite3_vec_init;
use thiserror::Error;

static REGISTER_SQLITE_VEC: Once = Once::new();

#[derive(Clone, Debug, PartialEq)]
pub struct TextDocumentRecord {
    pub namespace: String,
    pub kind: String,
    pub document_id: String,
    pub content: String,
    pub updated_at: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextVectorRecord {
    pub document: TextDocumentRecord,
    pub embedding: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DocumentState {
    pub row_id: i64,
    pub namespace: String,
    pub kind: String,
    pub document_id: String,
    pub content: String,
    pub revision: i64,
    pub updated_at: i64,
    pub embedded_revision: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextSearchResult {
    pub document_row_id: i64,
    pub namespace: String,
    pub kind: String,
    pub document_id: String,
    pub content: String,
    pub revision: i64,
    pub updated_at: i64,
    pub distance: f64,
    pub similarity: f64,
}

/// Read-only index counts used by the semantic-search availability UI.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TextVectorStoreStatus {
    pub tag_document_count: u64,
    pub tag_indexed_count: u64,
    pub tag_link_count: u64,
    pub annotation_document_count: u64,
    pub annotation_indexed_count: u64,
    pub legacy_model_detected: bool,
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

    #[error("namespace, kind, and document_id cannot be empty")]
    EmptyDocumentKey,

    #[error("document content cannot be empty")]
    EmptyContent,

    #[error("item_id cannot be empty")]
    EmptyItemId,

    #[error("text document `{document_id}` was not found in {namespace}/{kind}")]
    DocumentNotFound {
        namespace: String,
        kind: String,
        document_id: String,
    },

    #[error("top_k must be greater than zero")]
    InvalidTopK,

    #[error("document revision overflowed")]
    RevisionOverflow,

    #[error("failed to create text vector database directory {path}: {source}")]
    CreateDirectory {
        path: std::path::PathBuf,
        source: std::io::Error,
    },

    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

pub struct TextVectorStore {
    connection: Connection,
    model_id: i64,
    model_key: String,
    vector_table: String,
    dimension: usize,
}

impl TextVectorStore {
    pub fn stored_dimension(
        path: impl AsRef<Path>,
        model_key: &str,
    ) -> Result<Option<usize>, StoreError> {
        let path = path.as_ref();
        if !path.exists() || model_key.trim().is_empty() {
            return Ok(None);
        }
        let connection = Connection::open(path)?;
        let table_exists = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'text_vector_models')",
            [],
            |row| row.get::<_, i64>(0),
        )?;
        if table_exists == 0 {
            return Ok(None);
        }
        Ok(connection
            .query_row(
                "SELECT dimension FROM text_vector_models WHERE model_key = ?1",
                [model_key],
                |row| row.get::<_, usize>(0),
            )
            .optional()?)
    }

    pub fn read_status(
        path: impl AsRef<Path>,
        model_key: &str,
        legacy_model_key: &str,
        namespace: &str,
    ) -> Result<TextVectorStoreStatus, StoreError> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(TextVectorStoreStatus::default());
        }
        let connection = Connection::open(path)?;
        let table_exists = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'text_documents')",
            [],
            |row| row.get::<_, i64>(0),
        )?;
        if table_exists == 0 {
            return Ok(TextVectorStoreStatus::default());
        }
        let tag_document_count = count_documents_readonly(&connection, namespace, "tag")?;
        let annotation_document_count =
            count_documents_readonly(&connection, namespace, "annotation")?;
        let tag_link_count = count_links_readonly(&connection, namespace, "tag")?;
        let legacy_model_detected = has_unmigrated_embeddings(
            &connection,
            namespace,
            "tag",
            legacy_model_key,
            &format!("{model_key}::tag"),
        )? || has_unmigrated_embeddings(
            &connection,
            namespace,
            "annotation",
            legacy_model_key,
            &format!("{model_key}::annotation"),
        )? || has_unmigrated_embeddings(
            &connection,
            namespace,
            "tag",
            model_key,
            &format!("{model_key}::tag"),
        )? || has_unmigrated_embeddings(
            &connection,
            namespace,
            "annotation",
            model_key,
            &format!("{model_key}::annotation"),
        )?;
        let tag_indexed_count = count_embeddings_prefer_current(
            &connection,
            namespace,
            "tag",
            model_key,
            legacy_model_key,
        )?;
        let annotation_indexed_count = count_embeddings_prefer_current(
            &connection,
            namespace,
            "annotation",
            model_key,
            legacy_model_key,
        )?;
        Ok(TextVectorStoreStatus {
            tag_document_count,
            tag_indexed_count,
            tag_link_count,
            annotation_document_count,
            annotation_indexed_count,
            legacy_model_detected,
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
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS text_documents (
                 id INTEGER PRIMARY KEY,
                 namespace TEXT NOT NULL,
                 kind TEXT NOT NULL,
                 document_id TEXT NOT NULL,
                 content TEXT NOT NULL,
                 revision INTEGER NOT NULL CHECK (revision > 0),
                 updated_at INTEGER NOT NULL,
                 UNIQUE(namespace, kind, document_id)
             );
             CREATE INDEX IF NOT EXISTS text_documents_scope
                 ON text_documents(namespace, kind);
             CREATE TABLE IF NOT EXISTS text_document_links (
                 document_row_id INTEGER NOT NULL
                     REFERENCES text_documents(id) ON DELETE CASCADE,
                 item_id TEXT NOT NULL,
                 PRIMARY KEY(document_row_id, item_id)
             );
             CREATE INDEX IF NOT EXISTS text_document_links_item
                 ON text_document_links(item_id, document_row_id);
             CREATE TABLE IF NOT EXISTS text_vector_models (
                 id INTEGER PRIMARY KEY,
                 model_key TEXT NOT NULL UNIQUE,
                 dimension INTEGER NOT NULL CHECK (dimension > 0)
             );
             CREATE TABLE IF NOT EXISTS text_model_embeddings (
                 id INTEGER PRIMARY KEY,
                 model_id INTEGER NOT NULL
                     REFERENCES text_vector_models(id) ON DELETE CASCADE,
                 document_row_id INTEGER NOT NULL
                     REFERENCES text_documents(id) ON DELETE CASCADE,
                 document_revision INTEGER NOT NULL CHECK (document_revision > 0),
                 UNIQUE(model_id, document_row_id)
             );
             CREATE INDEX IF NOT EXISTS text_model_embeddings_document
                 ON text_model_embeddings(document_row_id, model_id);",
        )?;

        connection.execute(
            "INSERT INTO text_vector_models(model_key, dimension) VALUES (?1, ?2)
             ON CONFLICT(model_key) DO NOTHING",
            params![model_key, dimension],
        )?;
        let (model_id, actual) = connection.query_row(
            "SELECT id, dimension FROM text_vector_models WHERE model_key = ?1",
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

        let vector_table = vector_table_name(model_id);
        connection.execute(
            &format!(
                "CREATE VIRTUAL TABLE IF NOT EXISTS {vector_table} USING vec0(
                    embedding float[{dimension}] distance_metric=cosine,
                    namespace text,
                    kind text
                )"
            ),
            [],
        )?;

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

    pub fn merge_model(&mut self, source_model_key: &str) -> Result<u64, StoreError> {
        self.merge_model_filtered(source_model_key, None, &mut |_, _| {})
    }

    pub fn merge_model_kind(
        &mut self,
        source_model_key: &str,
        kind: &str,
    ) -> Result<u64, StoreError> {
        let kind = kind.trim();
        if kind.is_empty() {
            return Ok(0);
        }
        self.merge_model_filtered(source_model_key, Some(kind), &mut |_, _| {})
    }

    pub fn merge_model_kind_with_progress(
        &mut self,
        source_model_key: &str,
        kind: &str,
        mut on_progress: impl FnMut(u64, u64),
    ) -> Result<u64, StoreError> {
        let kind = kind.trim();
        if kind.is_empty() {
            return Ok(0);
        }
        self.merge_model_filtered(source_model_key, Some(kind), &mut on_progress)
    }

    pub fn pending_merge_count(
        &self,
        source_model_key: &str,
        kind: &str,
    ) -> Result<u64, StoreError> {
        let source_model_key = source_model_key.trim();
        let kind = kind.trim();
        if source_model_key.is_empty() || source_model_key == self.model_key || kind.is_empty() {
            return Ok(0);
        }
        let source = self
            .connection
            .query_row(
                "SELECT id, dimension FROM text_vector_models WHERE model_key = ?1",
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
        let source_table = vector_table_name(source_id);
        Ok(self.connection.query_row(
            &format!(
                "SELECT COUNT(*)
                 FROM text_model_embeddings e
                 JOIN text_documents d ON d.id = e.document_row_id
                 JOIN {source_table} v ON v.rowid = e.id
                 LEFT JOIN text_model_embeddings target
                   ON target.model_id = ?2
                  AND target.document_row_id = e.document_row_id
                 WHERE e.model_id = ?1 AND d.kind = ?3
                   AND (target.id IS NULL OR e.document_revision > target.document_revision)"
            ),
            params![source_id, self.model_id, kind],
            |row| row.get(0),
        )?)
    }

    fn merge_model_filtered(
        &mut self,
        source_model_key: &str,
        kind: Option<&str>,
        on_progress: &mut dyn FnMut(u64, u64),
    ) -> Result<u64, StoreError> {
        let source_model_key = source_model_key.trim();
        if source_model_key.is_empty() || source_model_key == self.model_key {
            return Ok(0);
        }
        let kind = kind.unwrap_or("");
        let source = self
            .connection
            .query_row(
                "SELECT id, dimension FROM text_vector_models WHERE model_key = ?1",
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
        let source_table = vector_table_name(source_id);
        let rows = {
            let mut statement = self.connection.prepare(&format!(
                "SELECT e.document_row_id, e.document_revision,
                        d.namespace, d.kind, v.embedding
                 FROM text_model_embeddings e
                 JOIN text_documents d ON d.id = e.document_row_id
                 JOIN {source_table} v ON v.rowid = e.id
                 LEFT JOIN text_model_embeddings target
                   ON target.model_id = ?2
                  AND target.document_row_id = e.document_row_id
                 WHERE e.model_id = ?1
                   AND (?3 = '' OR d.kind = ?3)
                   AND (target.id IS NULL OR e.document_revision > target.document_revision)"
            ))?;
            statement
                .query_map(params![source_id, self.model_id, kind], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Vec<u8>>(4)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?
        };
        let transaction = self.connection.transaction()?;
        let total = rows.len() as u64;
        for (index, (document_row_id, document_revision, namespace, kind, embedding)) in
            rows.iter().enumerate()
        {
            transaction.execute(
                "INSERT INTO text_model_embeddings(model_id, document_row_id, document_revision)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(model_id, document_row_id) DO UPDATE SET
                     document_revision = excluded.document_revision",
                params![self.model_id, document_row_id, document_revision],
            )?;
            let row_id = transaction.query_row(
                "SELECT id FROM text_model_embeddings
                 WHERE model_id = ?1 AND document_row_id = ?2",
                params![self.model_id, document_row_id],
                |row| row.get::<_, i64>(0),
            )?;
            transaction.execute(
                &format!("DELETE FROM {} WHERE rowid = ?1", self.vector_table),
                [row_id],
            )?;
            transaction.execute(
                &format!(
                    "INSERT INTO {}(rowid, embedding, namespace, kind)
                     VALUES (?1, ?2, ?3, ?4)",
                    self.vector_table
                ),
                params![row_id, embedding, namespace, kind],
            )?;
            on_progress(index as u64 + 1, total);
        }
        transaction.commit()?;
        Ok(total)
    }

    pub fn sqlite_vec_version(&self) -> Result<String, StoreError> {
        Ok(self
            .connection
            .query_row("SELECT vec_version()", [], |row| row.get(0))?)
    }

    pub fn upsert_document(
        &mut self,
        record: &TextDocumentRecord,
    ) -> Result<DocumentState, StoreError> {
        validate_document(record)?;
        let transaction = self.connection.transaction()?;
        upsert_document_metadata(&transaction, record)?;
        transaction.commit()?;
        self.document_state(&record.namespace, &record.kind, &record.document_id)?
            .ok_or_else(|| document_not_found(record))
    }

    pub fn upsert_documents(
        &mut self,
        records: &[TextDocumentRecord],
    ) -> Result<Vec<DocumentState>, StoreError> {
        for record in records {
            validate_document(record)?;
        }
        let transaction = self.connection.transaction()?;
        for record in records {
            upsert_document_metadata(&transaction, record)?;
        }
        transaction.commit()?;
        records
            .iter()
            .map(|record| {
                self.document_state(&record.namespace, &record.kind, &record.document_id)?
                    .ok_or_else(|| document_not_found(record))
            })
            .collect()
    }

    pub fn upsert(&mut self, record: &TextVectorRecord) -> Result<i64, StoreError> {
        validate_document(&record.document)?;
        validate_embedding(&record.embedding, self.dimension)?;
        let transaction = self.connection.transaction()?;
        let document = upsert_document_metadata(&transaction, &record.document)?;
        let row_id = upsert_embedding(
            &transaction,
            self.model_id,
            &self.vector_table,
            &document,
            &record.embedding,
        )?;
        transaction.commit()?;
        Ok(row_id)
    }

    pub fn upsert_many(&mut self, records: &[TextVectorRecord]) -> Result<Vec<i64>, StoreError> {
        for record in records {
            validate_document(&record.document)?;
            validate_embedding(&record.embedding, self.dimension)?;
        }
        let transaction = self.connection.transaction()?;
        let mut row_ids = Vec::with_capacity(records.len());
        for record in records {
            let document = upsert_document_metadata(&transaction, &record.document)?;
            row_ids.push(upsert_embedding(
                &transaction,
                self.model_id,
                &self.vector_table,
                &document,
                &record.embedding,
            )?);
        }
        transaction.commit()?;
        Ok(row_ids)
    }

    pub fn upsert_document_embedding(
        &mut self,
        namespace: &str,
        kind: &str,
        document_id: &str,
        embedding: &[f32],
    ) -> Result<i64, StoreError> {
        validate_document_key(namespace, kind, document_id)?;
        validate_embedding(embedding, self.dimension)?;
        let transaction = self.connection.transaction()?;
        let document =
            find_document(&transaction, namespace, kind, document_id)?.ok_or_else(|| {
                StoreError::DocumentNotFound {
                    namespace: namespace.to_string(),
                    kind: kind.to_string(),
                    document_id: document_id.to_string(),
                }
            })?;
        let row_id = upsert_embedding(
            &transaction,
            self.model_id,
            &self.vector_table,
            &document,
            embedding,
        )?;
        transaction.commit()?;
        Ok(row_id)
    }

    pub fn document_state(
        &self,
        namespace: &str,
        kind: &str,
        document_id: &str,
    ) -> Result<Option<DocumentState>, StoreError> {
        validate_document_key(namespace, kind, document_id)?;
        Ok(self
            .connection
            .query_row(
                "SELECT d.id, d.namespace, d.kind, d.document_id, d.content,
                        d.revision, d.updated_at, e.document_revision
                 FROM text_documents d
                 LEFT JOIN text_model_embeddings e
                   ON e.document_row_id = d.id AND e.model_id = ?1
                 WHERE d.namespace = ?2 AND d.kind = ?3 AND d.document_id = ?4",
                params![self.model_id, namespace, kind, document_id],
                map_document_state,
            )
            .optional()?)
    }

    pub fn needs_embedding(
        &self,
        namespace: &str,
        kind: &str,
        document_id: &str,
        content: &str,
    ) -> Result<bool, StoreError> {
        Ok(match self.document_state(namespace, kind, document_id)? {
            Some(state) => {
                state.content != content || state.embedded_revision != Some(state.revision)
            }
            None => true,
        })
    }

    pub fn replace_item_links(
        &mut self,
        namespace: &str,
        kind: &str,
        item_id: &str,
        document_ids: &[String],
    ) -> Result<usize, StoreError> {
        validate_scope(namespace, kind)?;
        if item_id.trim().is_empty() {
            return Err(StoreError::EmptyItemId);
        }
        let unique_ids = document_ids
            .iter()
            .map(|document_id| document_id.trim())
            .filter(|document_id| !document_id.is_empty())
            .collect::<HashSet<_>>();
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "DELETE FROM text_document_links
             WHERE item_id = ?1 AND document_row_id IN (
                 SELECT id FROM text_documents WHERE namespace = ?2 AND kind = ?3
             )",
            params![item_id, namespace, kind],
        )?;
        for document_id in &unique_ids {
            let row_id = transaction
                .query_row(
                    "SELECT id FROM text_documents
                     WHERE namespace = ?1 AND kind = ?2 AND document_id = ?3",
                    params![namespace, kind, document_id],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?
                .ok_or_else(|| StoreError::DocumentNotFound {
                    namespace: namespace.to_string(),
                    kind: kind.to_string(),
                    document_id: (*document_id).to_string(),
                })?;
            transaction.execute(
                "INSERT INTO text_document_links(document_row_id, item_id)
                 VALUES (?1, ?2)",
                params![row_id, item_id],
            )?;
        }
        transaction.commit()?;
        Ok(unique_ids.len())
    }

    pub fn prune_item_links(
        &mut self,
        namespace: &str,
        kind: &str,
        keep_item_ids: &HashSet<String>,
    ) -> Result<u64, StoreError> {
        validate_scope(namespace, kind)?;
        let transaction = self.connection.transaction()?;
        let stale_links = {
            let mut statement = transaction.prepare(
                "SELECT l.document_row_id, l.item_id
                 FROM text_document_links l
                 JOIN text_documents d ON d.id = l.document_row_id
                 WHERE d.namespace = ?1 AND d.kind = ?2",
            )?;
            statement
                .query_map(params![namespace, kind], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })?
                .filter_map(|row| match row {
                    Ok((document_row_id, item_id)) if !keep_item_ids.contains(&item_id) => {
                        Some(Ok((document_row_id, item_id)))
                    }
                    Ok(_) => None,
                    Err(error) => Some(Err(error)),
                })
                .collect::<Result<Vec<_>, _>>()?
        };

        for (document_row_id, item_id) in &stale_links {
            transaction.execute(
                "DELETE FROM text_document_links
                 WHERE document_row_id = ?1 AND item_id = ?2",
                params![document_row_id, item_id],
            )?;
        }
        transaction.commit()?;
        Ok(stale_links.len() as u64)
    }

    pub fn linked_item_ids(&self, document_row_id: i64) -> Result<Vec<String>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT item_id FROM text_document_links
             WHERE document_row_id = ?1 ORDER BY item_id",
        )?;
        Ok(statement
            .query_map([document_row_id], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn linked_item_ids_by_key(
        &self,
        namespace: &str,
        kind: &str,
        document_id: &str,
    ) -> Result<Vec<String>, StoreError> {
        validate_document_key(namespace, kind, document_id)?;
        let row_id = self
            .connection
            .query_row(
                "SELECT id FROM text_documents
                 WHERE namespace = ?1 AND kind = ?2 AND document_id = ?3",
                params![namespace, kind, document_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .ok_or_else(|| StoreError::DocumentNotFound {
                namespace: namespace.to_string(),
                kind: kind.to_string(),
                document_id: document_id.to_string(),
            })?;
        self.linked_item_ids(row_id)
    }

    pub fn search(
        &self,
        namespace: &str,
        kind: &str,
        query: &[f32],
        top_k: usize,
    ) -> Result<Vec<TextSearchResult>, StoreError> {
        validate_scope(namespace, kind)?;
        validate_embedding(query, self.dimension)?;
        if top_k == 0 {
            return Err(StoreError::InvalidTopK);
        }
        let query_blob = embedding_blob(query);
        let mut statement = self.connection.prepare(&format!(
            "SELECT d.id, d.namespace, d.kind, d.document_id, d.content,
                    d.revision, d.updated_at, v.distance
             FROM {} v
             JOIN text_model_embeddings e ON e.id = v.rowid
             JOIN text_documents d ON d.id = e.document_row_id
             WHERE v.embedding MATCH ?1 AND k = ?2
               AND v.namespace = ?3 AND v.kind = ?4
               AND e.model_id = ?5 AND e.document_revision = d.revision
             ORDER BY v.distance",
            self.vector_table
        ))?;
        Ok(statement
            .query_map(
                params![query_blob, top_k as i64, namespace, kind, self.model_id],
                map_search_row,
            )?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn delete_document(
        &mut self,
        namespace: &str,
        kind: &str,
        document_id: &str,
    ) -> Result<bool, StoreError> {
        validate_document_key(namespace, kind, document_id)?;
        let transaction = self.connection.transaction()?;
        let row_id = transaction
            .query_row(
                "SELECT id FROM text_documents
                 WHERE namespace = ?1 AND kind = ?2 AND document_id = ?3",
                params![namespace, kind, document_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        let Some(row_id) = row_id else {
            return Ok(false);
        };
        delete_embeddings_for_document(&transaction, row_id)?;
        transaction.execute("DELETE FROM text_documents WHERE id = ?1", [row_id])?;
        transaction.commit()?;
        Ok(true)
    }

    pub fn prune_unlinked_documents(
        &mut self,
        namespace: &str,
        kind: &str,
    ) -> Result<u64, StoreError> {
        validate_scope(namespace, kind)?;
        let transaction = self.connection.transaction()?;
        let row_ids = {
            let mut statement = transaction.prepare(
                "SELECT d.id FROM text_documents d
                 WHERE d.namespace = ?1 AND d.kind = ?2
                   AND NOT EXISTS (
                       SELECT 1 FROM text_document_links l
                       WHERE l.document_row_id = d.id
                   )",
            )?;
            statement
                .query_map(params![namespace, kind], |row| row.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()?
        };
        for row_id in &row_ids {
            delete_embeddings_for_document(&transaction, *row_id)?;
            transaction.execute("DELETE FROM text_documents WHERE id = ?1", [row_id])?;
        }
        transaction.commit()?;
        Ok(row_ids.len() as u64)
    }

    pub fn count_documents(&self, namespace: &str, kind: &str) -> Result<u64, StoreError> {
        validate_scope(namespace, kind)?;
        Ok(self.connection.query_row(
            "SELECT COUNT(*) FROM text_documents WHERE namespace = ?1 AND kind = ?2",
            params![namespace, kind],
            |row| row.get(0),
        )?)
    }

    pub fn count_embeddings(&self, namespace: &str, kind: &str) -> Result<u64, StoreError> {
        validate_scope(namespace, kind)?;
        Ok(self.connection.query_row(
            "SELECT COUNT(*)
             FROM text_model_embeddings e
             JOIN text_documents d ON d.id = e.document_row_id
             WHERE e.model_id = ?1 AND d.namespace = ?2 AND d.kind = ?3
               AND e.document_revision = d.revision",
            params![self.model_id, namespace, kind],
            |row| row.get(0),
        )?)
    }

    pub fn count_links(&self, namespace: &str, kind: &str) -> Result<u64, StoreError> {
        validate_scope(namespace, kind)?;
        Ok(self.connection.query_row(
            "SELECT COUNT(*)
             FROM text_document_links l
             JOIN text_documents d ON d.id = l.document_row_id
             WHERE d.namespace = ?1 AND d.kind = ?2",
            params![namespace, kind],
            |row| row.get(0),
        )?)
    }
}

fn count_documents_readonly(
    connection: &Connection,
    namespace: &str,
    kind: &str,
) -> Result<u64, StoreError> {
    Ok(connection.query_row(
        "SELECT COUNT(*) FROM text_documents WHERE namespace = ?1 AND kind = ?2",
        params![namespace, kind],
        |row| row.get(0),
    )?)
}

fn count_links_readonly(
    connection: &Connection,
    namespace: &str,
    kind: &str,
) -> Result<u64, StoreError> {
    Ok(connection.query_row(
        "SELECT COUNT(*)
         FROM text_document_links l
         JOIN text_documents d ON d.id = l.document_row_id
         WHERE d.namespace = ?1 AND d.kind = ?2",
        params![namespace, kind],
        |row| row.get(0),
    )?)
}

fn count_embeddings_for_model(
    connection: &Connection,
    namespace: &str,
    kind: &str,
    model_key: &str,
) -> Result<u64, StoreError> {
    let Some(model_id) = connection
        .query_row(
            "SELECT id FROM text_vector_models WHERE model_key = ?1",
            [model_key],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
    else {
        return Ok(0);
    };
    Ok(connection.query_row(
        "SELECT COUNT(*)
         FROM text_model_embeddings e
         JOIN text_documents d ON d.id = e.document_row_id
         WHERE e.model_id = ?1 AND d.namespace = ?2 AND d.kind = ?3
           AND e.document_revision = d.revision",
        params![model_id, namespace, kind],
        |row| row.get(0),
    )?)
}

fn has_unmigrated_embeddings(
    connection: &Connection,
    namespace: &str,
    kind: &str,
    source_model_key: &str,
    target_model_key: &str,
) -> Result<bool, StoreError> {
    if source_model_key.trim().is_empty() || source_model_key == target_model_key {
        return Ok(false);
    }
    let Some(source_model_id) = connection
        .query_row(
            "SELECT id FROM text_vector_models WHERE model_key = ?1",
            [source_model_key],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
    else {
        return Ok(false);
    };
    let target_model_id = connection
        .query_row(
            "SELECT id FROM text_vector_models WHERE model_key = ?1",
            [target_model_key],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    let Some(target_model_id) = target_model_id else {
        return Ok(count_embeddings_for_model(connection, namespace, kind, source_model_key)? > 0);
    };
    Ok(connection.query_row(
        "SELECT EXISTS(
             SELECT 1
             FROM text_model_embeddings source
             JOIN text_documents d ON d.id = source.document_row_id
             LEFT JOIN text_model_embeddings target
               ON target.model_id = ?2
              AND target.document_row_id = source.document_row_id
              AND target.document_revision >= source.document_revision
             WHERE source.model_id = ?1
               AND d.namespace = ?3 AND d.kind = ?4
               AND source.document_revision = d.revision
               AND target.id IS NULL
         )",
        params![source_model_id, target_model_id, namespace, kind],
        |row| row.get::<_, bool>(0),
    )?)
}

fn count_embeddings_prefer_current(
    connection: &Connection,
    namespace: &str,
    kind: &str,
    model_key: &str,
    legacy_model_key: &str,
) -> Result<u64, StoreError> {
    let candidates = [
        format!("{model_key}::{kind}"),
        format!("{legacy_model_key}::{kind}"),
    ];
    for (index, candidate) in candidates.iter().enumerate() {
        if candidate.is_empty() || candidates[..index].contains(candidate) {
            continue;
        }
        let count = count_embeddings_for_model(connection, namespace, kind, candidate)?;
        if count > 0 {
            return Ok(count);
        }
    }
    Ok(0)
}

#[derive(Clone, Debug)]
struct StoredDocument {
    row_id: i64,
    namespace: String,
    kind: String,
    revision: i64,
}

fn upsert_document_metadata(
    transaction: &Transaction<'_>,
    record: &TextDocumentRecord,
) -> Result<StoredDocument, StoreError> {
    let existing = transaction
        .query_row(
            "SELECT id, content, revision FROM text_documents
             WHERE namespace = ?1 AND kind = ?2 AND document_id = ?3",
            params![record.namespace, record.kind, record.document_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()?;

    let (row_id, revision) = if let Some((row_id, content, revision)) = existing {
        let content_changed = content != record.content;
        let next_revision = if content_changed {
            revision
                .checked_add(1)
                .ok_or(StoreError::RevisionOverflow)?
        } else {
            revision
        };
        transaction.execute(
            "UPDATE text_documents
             SET content = ?1, revision = ?2, updated_at = ?3
             WHERE id = ?4",
            params![record.content, next_revision, record.updated_at, row_id],
        )?;
        if content_changed {
            delete_embeddings_for_document(transaction, row_id)?;
        }
        (row_id, next_revision)
    } else {
        transaction.execute(
            "INSERT INTO text_documents(
                 namespace, kind, document_id, content, revision, updated_at
             ) VALUES (?1, ?2, ?3, ?4, 1, ?5)",
            params![
                record.namespace,
                record.kind,
                record.document_id,
                record.content,
                record.updated_at
            ],
        )?;
        (transaction.last_insert_rowid(), 1)
    };

    Ok(StoredDocument {
        row_id,
        namespace: record.namespace.clone(),
        kind: record.kind.clone(),
        revision,
    })
}

fn find_document(
    transaction: &Transaction<'_>,
    namespace: &str,
    kind: &str,
    document_id: &str,
) -> Result<Option<StoredDocument>, StoreError> {
    Ok(transaction
        .query_row(
            "SELECT id, namespace, kind, revision
             FROM text_documents
             WHERE namespace = ?1 AND kind = ?2 AND document_id = ?3",
            params![namespace, kind, document_id],
            |row| {
                Ok(StoredDocument {
                    row_id: row.get(0)?,
                    namespace: row.get(1)?,
                    kind: row.get(2)?,
                    revision: row.get(3)?,
                })
            },
        )
        .optional()?)
}

fn upsert_embedding(
    transaction: &Transaction<'_>,
    model_id: i64,
    vector_table: &str,
    document: &StoredDocument,
    embedding: &[f32],
) -> Result<i64, StoreError> {
    transaction.execute(
        "INSERT INTO text_model_embeddings(model_id, document_row_id, document_revision)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(model_id, document_row_id) DO UPDATE SET
             document_revision = excluded.document_revision",
        params![model_id, document.row_id, document.revision],
    )?;
    let row_id = transaction.query_row(
        "SELECT id FROM text_model_embeddings
         WHERE model_id = ?1 AND document_row_id = ?2",
        params![model_id, document.row_id],
        |row| row.get::<_, i64>(0),
    )?;
    transaction.execute(
        &format!("DELETE FROM {vector_table} WHERE rowid = ?1"),
        [row_id],
    )?;
    transaction.execute(
        &format!(
            "INSERT INTO {vector_table}(rowid, embedding, namespace, kind)
             VALUES (?1, ?2, ?3, ?4)"
        ),
        params![
            row_id,
            embedding_blob(embedding),
            document.namespace,
            document.kind
        ],
    )?;
    Ok(row_id)
}

fn delete_embeddings_for_document(
    transaction: &Transaction<'_>,
    document_row_id: i64,
) -> Result<(), StoreError> {
    let embeddings = {
        let mut statement = transaction
            .prepare("SELECT id, model_id FROM text_model_embeddings WHERE document_row_id = ?1")?;
        statement
            .query_map([document_row_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    for (row_id, model_id) in embeddings {
        transaction.execute(
            &format!(
                "DELETE FROM {} WHERE rowid = ?1",
                vector_table_name(model_id)
            ),
            [row_id],
        )?;
    }
    transaction.execute(
        "DELETE FROM text_model_embeddings WHERE document_row_id = ?1",
        [document_row_id],
    )?;
    Ok(())
}

fn map_document_state(row: &rusqlite::Row<'_>) -> rusqlite::Result<DocumentState> {
    Ok(DocumentState {
        row_id: row.get(0)?,
        namespace: row.get(1)?,
        kind: row.get(2)?,
        document_id: row.get(3)?,
        content: row.get(4)?,
        revision: row.get(5)?,
        updated_at: row.get(6)?,
        embedded_revision: row.get(7)?,
    })
}

fn map_search_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TextSearchResult> {
    let distance = row.get::<_, f64>(7)?;
    Ok(TextSearchResult {
        document_row_id: row.get(0)?,
        namespace: row.get(1)?,
        kind: row.get(2)?,
        document_id: row.get(3)?,
        content: row.get(4)?,
        revision: row.get(5)?,
        updated_at: row.get(6)?,
        distance,
        similarity: 1.0 - distance,
    })
}

fn validate_model(model_key: String, dimension: usize) -> Result<String, StoreError> {
    if model_key.trim().is_empty() {
        return Err(StoreError::EmptyModelKey);
    }
    if dimension == 0 {
        return Err(StoreError::InvalidDimension);
    }
    Ok(model_key)
}

fn validate_document(record: &TextDocumentRecord) -> Result<(), StoreError> {
    validate_document_key(&record.namespace, &record.kind, &record.document_id)?;
    if record.content.trim().is_empty() {
        return Err(StoreError::EmptyContent);
    }
    Ok(())
}

fn validate_scope(namespace: &str, kind: &str) -> Result<(), StoreError> {
    if namespace.trim().is_empty() || kind.trim().is_empty() {
        return Err(StoreError::EmptyDocumentKey);
    }
    Ok(())
}

fn validate_document_key(namespace: &str, kind: &str, document_id: &str) -> Result<(), StoreError> {
    if namespace.trim().is_empty() || kind.trim().is_empty() || document_id.trim().is_empty() {
        return Err(StoreError::EmptyDocumentKey);
    }
    Ok(())
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

fn document_not_found(record: &TextDocumentRecord) -> StoreError {
    StoreError::DocumentNotFound {
        namespace: record.namespace.clone(),
        kind: record.kind.clone(),
        document_id: record.document_id.clone(),
    }
}

fn vector_table_name(model_id: i64) -> String {
    format!("text_vectors_{model_id}")
}

fn embedding_blob(embedding: &[f32]) -> Vec<u8> {
    embedding
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
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

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use vector_store::{Modality, VectorRecord, VectorStore};

    use super::*;

    static UNIQUE: AtomicU64 = AtomicU64::new(0);

    struct TempDatabase {
        path: PathBuf,
    }

    impl TempDatabase {
        fn new(name: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let unique = UNIQUE.fetch_add(1, Ordering::Relaxed);
            Self {
                path: std::env::temp_dir().join(format!(
                    "text-vector-store-{name}-{}-{nanos}-{unique}.sqlite3",
                    std::process::id()
                )),
            }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDatabase {
        fn drop(&mut self) {
            for suffix in ["", "-wal", "-shm"] {
                let _ = fs::remove_file(format!("{}{}", self.path.display(), suffix));
            }
        }
    }

    fn document(id: &str, content: &str) -> TextDocumentRecord {
        TextDocumentRecord {
            namespace: "library-a".to_string(),
            kind: "tag".to_string(),
            document_id: id.to_string(),
            content: content.to_string(),
            updated_at: 1,
        }
    }

    fn vector(id: &str, content: &str, embedding: Vec<f32>) -> TextVectorRecord {
        TextVectorRecord {
            document: document(id, content),
            embedding,
        }
    }

    #[test]
    fn shares_a_database_without_touching_image_vector_tables() {
        let database = TempDatabase::new("shared");
        let mut image_store = VectorStore::open(database.path(), "image-model", 2).unwrap();
        image_store
            .upsert(&VectorRecord {
                namespace: "library-a".to_string(),
                item_id: "image-1".to_string(),
                modality: Modality::Image,
                source_key: "primary".to_string(),
                source_uri: Some("image.png".to_string()),
                content: Some("image".to_string()),
                updated_at: 1,
                embedding: vec![1.0, 0.0],
            })
            .unwrap();

        let mut text_store = TextVectorStore::open(database.path(), "text-model", 2).unwrap();
        text_store
            .upsert(&vector("beach", "beach", vec![1.0, 0.0]))
            .unwrap();

        assert_eq!(image_store.count("library-a").unwrap(), 1);
        assert_eq!(text_store.count_documents("library-a", "tag").unwrap(), 1);
        assert_eq!(
            image_store
                .search("library-a", Some(Modality::Image), &[1.0, 0.0], 1)
                .unwrap()[0]
                .item_id,
            "image-1"
        );
        assert_eq!(
            text_store
                .search("library-a", "tag", &[1.0, 0.0], 1)
                .unwrap()[0]
                .document_id,
            "beach"
        );
    }

    #[test]
    fn search_respects_namespace_and_kind() {
        let mut store = TextVectorStore::open_in_memory("text-model", 2).unwrap();
        store
            .upsert(&vector("beach", "beach", vec![1.0, 0.0]))
            .unwrap();
        let mut annotation = vector("image-1", "beach annotation", vec![1.0, 0.0]);
        annotation.document.kind = "annotation".to_string();
        store.upsert(&annotation).unwrap();
        let mut other = vector("other", "other", vec![1.0, 0.0]);
        other.document.namespace = "library-b".to_string();
        store.upsert(&other).unwrap();

        let results = store.search("library-a", "tag", &[1.0, 0.0], 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].document_id, "beach");
        assert!(results[0].similarity > 0.999);
    }

    #[test]
    fn changing_item_tags_only_updates_links() {
        let mut store = TextVectorStore::open_in_memory("text-model", 2).unwrap();
        store
            .upsert_many(&[
                vector("beach", "beach", vec![1.0, 0.0]),
                vector("dance", "dance", vec![0.0, 1.0]),
            ])
            .unwrap();
        let beach_state = store
            .document_state("library-a", "tag", "beach")
            .unwrap()
            .unwrap();

        store
            .replace_item_links(
                "library-a",
                "tag",
                "image-1",
                &["beach".to_string(), "dance".to_string()],
            )
            .unwrap();
        store
            .replace_item_links("library-a", "tag", "image-1", &["dance".to_string()])
            .unwrap();

        assert!(
            store
                .linked_item_ids_by_key("library-a", "tag", "beach")
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            store
                .linked_item_ids_by_key("library-a", "tag", "dance")
                .unwrap(),
            ["image-1"]
        );
        assert_eq!(store.count_embeddings("library-a", "tag").unwrap(), 2);
        assert_eq!(
            store
                .document_state("library-a", "tag", "beach")
                .unwrap()
                .unwrap()
                .revision,
            beach_state.revision
        );
    }

    #[test]
    fn content_changes_invalidate_other_model_embeddings() {
        let database = TempDatabase::new("revision");
        let mut first = TextVectorStore::open(database.path(), "model-a", 2).unwrap();
        first
            .upsert(&vector("beach", "beach", vec![1.0, 0.0]))
            .unwrap();
        let mut second = TextVectorStore::open(database.path(), "model-b", 2).unwrap();
        second
            .upsert_document(&document("beach", "beach and coast"))
            .unwrap();

        assert!(
            first
                .search("library-a", "tag", &[1.0, 0.0], 10)
                .unwrap()
                .is_empty()
        );
        assert!(
            second
                .needs_embedding("library-a", "tag", "beach", "beach and coast")
                .unwrap()
        );

        second
            .upsert_document_embedding("library-a", "tag", "beach", &[0.0, 1.0])
            .unwrap();
        assert_eq!(
            second.search("library-a", "tag", &[0.0, 1.0], 1).unwrap()[0].content,
            "beach and coast"
        );
    }

    #[test]
    fn merges_embeddings_from_a_legacy_model_key() {
        let database = TempDatabase::new("import");
        let mut legacy = TextVectorStore::open(database.path(), "legacy", 2).unwrap();
        legacy
            .upsert(&vector("legacy", "legacy", vec![1.0, 0.0]))
            .unwrap();
        legacy
            .upsert(&vector("shared", "old shared", vec![1.0, 0.0]))
            .unwrap();
        let mut current = TextVectorStore::open(database.path(), "current", 2).unwrap();
        current
            .upsert(&vector("shared", "new shared", vec![0.0, 1.0]))
            .unwrap();
        assert_eq!(current.merge_model("legacy").unwrap(), 1);
        assert_eq!(current.merge_model("legacy").unwrap(), 0);
        assert_eq!(current.count_embeddings("library-a", "tag").unwrap(), 2);
        assert!(
            current
                .search("library-a", "tag", &[1.0, 0.0], 2)
                .unwrap()
                .iter()
                .any(|result| result.document_id == "legacy")
        );
        assert_eq!(
            current.search("library-a", "tag", &[0.0, 1.0], 1).unwrap()[0].document_id,
            "shared"
        );
    }

    #[test]
    fn merges_only_the_requested_document_kind() {
        let database = TempDatabase::new("kind-filter");
        let mut legacy = TextVectorStore::open(database.path(), "legacy", 2).unwrap();
        legacy
            .upsert(&vector("tag", "tag", vec![1.0, 0.0]))
            .unwrap();
        legacy
            .upsert(&TextVectorRecord {
                document: TextDocumentRecord {
                    namespace: "library-a".to_string(),
                    kind: "annotation".to_string(),
                    document_id: "image-1".to_string(),
                    content: "note".to_string(),
                    updated_at: 1,
                },
                embedding: vec![0.0, 1.0],
            })
            .unwrap();
        let mut current = TextVectorStore::open(database.path(), "current", 2).unwrap();
        assert_eq!(current.merge_model_kind("legacy", "tag").unwrap(), 1);
        assert_eq!(current.count_embeddings("library-a", "tag").unwrap(), 1);
        assert_eq!(
            current.count_embeddings("library-a", "annotation").unwrap(),
            0
        );
    }

    #[test]
    fn legacy_detection_clears_after_embeddings_are_migrated() {
        let database = TempDatabase::new("legacy-status");
        let mut legacy = TextVectorStore::open(database.path(), "legacy", 2).unwrap();
        legacy
            .upsert(&vector("legacy", "legacy", vec![1.0, 0.0]))
            .unwrap();

        let before =
            TextVectorStore::read_status(database.path(), "current", "legacy", "library-a")
                .unwrap();
        assert!(before.legacy_model_detected);

        let mut current = TextVectorStore::open(database.path(), "current::tag", 2).unwrap();
        assert_eq!(current.pending_merge_count("legacy", "tag").unwrap(), 1);
        let mut progress = Vec::new();
        assert_eq!(
            current
                .merge_model_kind_with_progress("legacy", "tag", |completed, total| {
                    progress.push((completed, total));
                })
                .unwrap(),
            1
        );
        assert_eq!(progress, [(1, 1)]);
        let after = TextVectorStore::read_status(database.path(), "current", "legacy", "library-a")
            .unwrap();
        assert!(!after.legacy_model_detected);
        assert_eq!(after.tag_indexed_count, 1);
    }

    #[test]
    fn prunes_only_unlinked_documents_in_the_requested_scope() {
        let mut store = TextVectorStore::open_in_memory("text-model", 2).unwrap();
        store
            .upsert_many(&[
                vector("beach", "beach", vec![1.0, 0.0]),
                vector("dance", "dance", vec![0.0, 1.0]),
            ])
            .unwrap();
        store
            .replace_item_links("library-a", "tag", "image-1", &["dance".to_string()])
            .unwrap();

        assert_eq!(
            store.prune_unlinked_documents("library-a", "tag").unwrap(),
            1
        );
        assert_eq!(store.count_documents("library-a", "tag").unwrap(), 1);
        assert_eq!(store.count_embeddings("library-a", "tag").unwrap(), 1);
    }

    #[test]
    fn prunes_links_for_items_missing_from_the_current_library() {
        let mut store = TextVectorStore::open_in_memory("text-model", 2).unwrap();
        store
            .upsert(&vector("beach", "beach", vec![1.0, 0.0]))
            .unwrap();
        store
            .replace_item_links("library-a", "tag", "deleted-image", &["beach".to_string()])
            .unwrap();
        store
            .replace_item_links("library-a", "tag", "current-image", &["beach".to_string()])
            .unwrap();

        let keep = ["current-image".to_string()].into_iter().collect();
        assert_eq!(
            store.prune_item_links("library-a", "tag", &keep).unwrap(),
            1
        );
        assert_eq!(
            store
                .linked_item_ids_by_key("library-a", "tag", "beach")
                .unwrap(),
            ["current-image"]
        );
    }
}
