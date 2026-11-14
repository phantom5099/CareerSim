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

/// S3 兼容对象存储适配器（MinIO / R2 / 七牛等均走同一套 S3 API）。
///
/// 固定 path-style 寻址，密钥只存在于客户端实例中，不进日志。
#[derive(Debug, Clone)]
pub struct S3AttachmentStore {
    client: aws_sdk_s3::Client,
    bucket: String,
}

impl S3AttachmentStore {
    /// Builds a client for a self-hosted or cloud S3-compatible endpoint.
    pub fn new(
        endpoint: &str,
        region: &str,
        bucket: impl Into<String>,
        access_key_id: &str,
        secret_access_key: &str,
    ) -> Self {
        let credentials = aws_sdk_s3::config::Credentials::new(
            access_key_id,
            secret_access_key,
            None,
            None,
            "careersim-static",
        );
        let config = aws_sdk_s3::config::Builder::new()
            .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
            .region(aws_sdk_s3::config::Region::new(region.to_owned()))
            .endpoint_url(endpoint)
            .credentials_provider(credentials)
            .force_path_style(true)
            .build();
        Self {
            client: aws_sdk_s3::Client::from_conf(config),
            bucket: bucket.into(),
        }
    }

    fn check(&self, key: &str) -> Result<(), AttachmentStoreError> {
        validate_storage_key(key)
    }
}

#[async_trait]
impl AttachmentStore for S3AttachmentStore {
    async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), AttachmentStoreError> {
        self.check(key)?;
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(aws_sdk_s3::primitives::ByteStream::from(bytes.to_vec()))
            .send()
            .await
            .map_err(|err| AttachmentStoreError::Backend {
                message: format!("s3 put_object failed: {err}"),
                source: None,
            })?;
        Ok(())
    }

    async fn open(
        &self,
        key: &str,
    ) -> Result<Box<dyn tokio::io::AsyncRead + Send + Unpin>, AttachmentStoreError> {
        self.check(key)?;
        match self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
        {
            Ok(resp) => {
                // 附件上限 10 MiB，整块读入内存再包一层内存 Reader。
                let data =
                    resp.body
                        .collect()
                        .await
                        .map_err(|err| AttachmentStoreError::Backend {
                            message: format!("s3 get_object stream failed: {err}"),
                            source: None,
                        })?;
                Ok(Box::new(std::io::Cursor::new(data.into_bytes().to_vec())))
            }
            Err(err) => {
                let is_missing = err
                    .as_service_error()
                    .is_some_and(|svc| svc.is_no_such_key());
                if is_missing {
                    return Err(AttachmentStoreError::NotFound);
                }
                Err(AttachmentStoreError::Backend {
                    message: format!("s3 get_object failed: {err}"),
                    source: None,
                })
            }
        }
    }

    async fn delete(&self, key: &str) -> Result<(), AttachmentStoreError> {
        self.check(key)?;
        // S3 语义：删除不存在的对象同样成功，无需特判。
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|err| AttachmentStoreError::Backend {
                message: format!("s3 delete_object failed: {err}"),
                source: None,
            })?;
        Ok(())
    }
}

/// Makes sure the given path is inside the expected directory (defense in
/// depth for adapters that build paths from keys).
pub fn ensure_path_within(dir: &Path, path: &Path) -> Result<(), AttachmentStoreError> {
    if !path.starts_with(dir) {
        return Err(AttachmentStoreError::InvalidKey);
    }
    Ok(())
}
