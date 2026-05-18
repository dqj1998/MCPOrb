use crate::error::OrbError;
use crate::format::Document;

pub mod markdown;
pub mod pdf;

pub struct ImportResult {
    pub document: Document,
    pub raw_chunks: Vec<RawChunk>,
}

pub struct RawChunk {
    pub text: String,
    pub page: Option<u32>,
    pub section_id: Option<u32>,
}

pub trait DocumentImporter {
    fn import(&self, path: &std::path::Path) -> Result<ImportResult, OrbError>;
    fn supported_extensions(&self) -> &[&str];
}
