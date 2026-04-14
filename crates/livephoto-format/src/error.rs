use std::io;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConformanceIssue {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid magic")]
    InvalidMagic,
    #[error("unsupported major version: {0}")]
    UnsupportedMajorVersion(u16),
    #[error("malformed header: {0}")]
    MalformedHeader(String),
    #[error("malformed TOC: {0}")]
    MalformedToc(String),
    #[error("duplicate chunk id: {0}")]
    DuplicateChunkId(u64),
    #[error("required chunk missing: {0}")]
    RequiredChunkMissing(&'static str),
    #[error("manifest parse failure: {0}")]
    ManifestParseFailure(String),
    #[error("manifest validation failure: {0}")]
    ManifestValidationFailure(String),
    #[error("checksum mismatch for chunk {chunk_id}")]
    ChecksumMismatch { chunk_id: u64 },
    #[error("invalid offset or length: {0}")]
    InvalidOffsetOrLength(String),
    #[error("unsupported codec: {0}")]
    UnsupportedCodec(String),
    #[error("io failure: {0}")]
    Io(#[from] io::Error),
    #[error("json failure: {0}")]
    Json(#[from] serde_json::Error),
}
