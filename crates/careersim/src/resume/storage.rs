//! Attachment binary storage: a port plus a local-filesystem adapter.

use std::fmt;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Maximum length accepted for a storage key.
const MAX_KEY_LEN: usize = 128;

/// Errors raised by attachment storage backends.
#[derive(Debug, Error)]
pub enum AttachmentStoreError {
    /// The provided storage key is malformed (possible path traversal).
    #[error("invalid attachment storage key")]
    InvalidKey,
    /// The stored attachment does not exist.
    #[error("attachment not found")]
    NotFound,
    /// The backend failed to complete the operation.
    #[error("attachment storage backend failure: {message}")]
    Backend {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

impl AttachmentStoreError {
    fn backend(message: impl fmt::Display) -> Self {
        AttachmentStoreError::Backend {
            message: message.to_string(),
            source: None,
        }
    }
}

/// Storage port for attachment binaries, addressed by opaque random keys.
#[async_trait]
pub trait AttachmentStore: Send + Sync {
    /// Writes the full byte payload under `key`. An existing file under the
    /// same key is replaced atomically (temp file + rename).
    async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), AttachmentStoreError>;

    /// Opens the stored bytes for reading.
    async fn open(
        &self,
        key: &str,
    ) -> Result<Box<dyn tokio::io::AsyncRead + Send + Unpin>, AttachmentStoreError>;

    /// Deletes the stored bytes. Deleting a missing key is a no-op.
    async fn delete(&self, key: &str) -> Result<(), AttachmentStoreError>;
}

/// Validates a storage key: non-empty, bounded length, and restricted to
/// `[A-Za-z0-9_-]` so path separators, `..`, and NUL bytes are impossible.
pub fn validate_storage_key(key: &str) -> Result<(), AttachmentStoreError> {
    if key.is_empty() || key.len() > MAX_KEY_LEN {
        return Err(AttachmentStoreError::InvalidKey);
    }
    for ch in key.chars() {
        let ok = ch.is_ascii_alphanumeric() || ch == '-' || ch == '_';
        if !ok {
            return Err(AttachmentStoreError::InvalidKey);
        }
    }
    Ok(())
}

/// Local filesystem adapter. Files live under a single root directory named
/// exactly by the (validated) storage key.
#[derive(Debug, Clone)]
pub struct LocalFsAttachmentStore {
    root: PathBuf,
}

impl LocalFsAttachmentStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn resolve(&self, key: &str) -> Result<PathBuf, AttachmentStoreError> {
        validate_storage_key(key)?;
        Ok(self.root.join(key))
    }
}

#[async_trait]
impl AttachmentStore for LocalFsAttachmentStore {
    async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), AttachmentStoreError> {
        let path = self.resolve(key)?;
        tokio::fs::create_dir_all(&self.root).await.map_err(|err| {
            AttachmentStoreError::Backend {
                message: "failed to create attachment directory".to_string(),
                source: Some(Box::new(err)),
            }
        })?;
        let tmp_path = self.root.join(format!("{key}.tmp"));
        {
            let mut file = tokio::fs::File::create(&tmp_path).await.map_err(|err| {
                AttachmentStoreError::Backend {
                    message: "failed to create attachment file".to_string(),
                    source: Some(Box::new(err)),
                }
            })?;
            file.write_all(bytes)
                .await
                .map_err(|err| AttachmentStoreError::Backend {
                    message: "failed to write attachment bytes".to_string(),
                    source: Some(Box::new(err)),
                })?;
            file.flush()
                .await
                .map_err(|err| AttachmentStoreError::Backend {
                    message: "failed to flush attachment bytes".to_string(),
                    source: Some(Box::new(err)),
                })?;
        }
        match tokio::fs::rename(&tmp_path, &path).await {
            Ok(()) => Ok(()),
            Err(err) => {
                let _ = tokio::fs::remove_file(&tmp_path).await;
                Err(AttachmentStoreError::Backend {
                    message: "failed to finalize attachment file".to_string(),
                    source: Some(Box::new(err)),
                })
            }
        }
    }

    async fn open(
        &self,
        key: &str,
    ) -> Result<Box<dyn tokio::io::AsyncRead + Send + Unpin>, AttachmentStoreError> {
        let path = self.resolve(key)?;
        match tokio::fs::File::open(&path).await {
            Ok(file) => Ok(Box::new(file)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Err(AttachmentStoreError::NotFound)
            }
            Err(err) => Err(AttachmentStoreError::Backend {
                message: "failed to open attachment file".to_string(),
                source: Some(Box::new(err)),
            }),
        }
    }

    async fn delete(&self, key: &str) -> Result<(), AttachmentStoreError> {
        let path = self.resolve(key)?;
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(AttachmentStoreError::Backend {
                message: "failed to delete attachment file".to_string(),
                source: Some(Box::new(err)),
            }),
        }
    }
}

/// Reads the full contents of an opened attachment (bounded by the 10 MiB
/// upload limit plus slack).
pub async fn read_attachment_bytes(
    reader: Box<dyn tokio::io::AsyncRead + Send + Unpin>,
) -> Result<Vec<u8>, AttachmentStoreError> {
    let mut bytes = Vec::new();
    reader
        .take((super::model::MAX_ATTACHMENT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .await
        .map_err(|err| AttachmentStoreError::Backend {
            message: "failed to read attachment bytes".to_string(),
            source: Some(Box::new(err)),
        })?;
    if bytes.len() as u64 > super::model::MAX_ATTACHMENT_BYTES as u64 {
        return Err(AttachmentStoreError::backend(
            "stored attachment exceeds the size limit",
        ));
    }
    Ok(bytes)
}

/// Makes sure the given path is inside the expected directory (defense in
/// depth for adapters that build paths from keys).
pub fn ensure_path_within(dir: &Path, path: &Path) -> Result<(), AttachmentStoreError> {
    if !path.starts_with(dir) {
        return Err(AttachmentStoreError::InvalidKey);
    }
    Ok(())
}
