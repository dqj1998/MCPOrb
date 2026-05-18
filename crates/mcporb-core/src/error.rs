use thiserror::Error;

#[derive(Debug, Error)]
pub enum OrbError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Document processing error: {0}")]
    DocumentProcessing(String),
    #[error("Index error: {0}")]
    Index(String),
}
