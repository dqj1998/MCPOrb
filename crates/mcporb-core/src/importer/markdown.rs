use std::path::Path;
use crate::error::OrbError;
use crate::format::{Document, Section};
use super::{DocumentImporter, ImportResult, RawChunk};

pub struct MarkdownImporter;

impl DocumentImporter for MarkdownImporter {
    fn import(&self, path: &Path) -> Result<ImportResult, OrbError> {
        let content = std::fs::read_to_string(path)
            .map_err(OrbError::Io)?;

        let title = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled")
            .to_string();

        let mut sections = Vec::new();
        let mut raw_chunks = Vec::new();
        let mut current_section_title = String::from("Introduction");
        let mut current_content = String::new();
        let mut section_id: u32 = 0;

        for line in content.lines() {
            if line.starts_with("## ") || line.starts_with("# ") {
                // Save previous section
                if !current_content.trim().is_empty() {
                    sections.push(Section {
                        id: section_id,
                        document_id: 0,
                        title: current_section_title.clone(),
                        page_start: None,
                        page_end: None,
                    });
                    raw_chunks.push(RawChunk {
                        text: current_content.trim().to_string(),
                        page: None,
                        section_id: Some(section_id),
                    });
                    section_id += 1;
                }
                current_section_title = line.trim_start_matches('#').trim().to_string();
                current_content = String::new();
            } else {
                current_content.push_str(line);
                current_content.push('\n');
            }
        }

        // Save last section
        if !current_content.trim().is_empty() {
            sections.push(Section {
                id: section_id,
                document_id: 0,
                title: current_section_title,
                page_start: None,
                page_end: None,
            });
            raw_chunks.push(RawChunk {
                text: current_content.trim().to_string(),
                page: None,
                section_id: Some(section_id),
            });
        }

        let document = Document {
            id: 0,
            title,
            source_path: path.to_string_lossy().to_string(),
            page_count: None,
            sections,
        };

        Ok(ImportResult { document, raw_chunks })
    }

    fn supported_extensions(&self) -> &[&str] {
        &["md", "markdown"]
    }
}
