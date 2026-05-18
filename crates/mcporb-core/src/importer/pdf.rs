use std::path::Path;
use crate::error::OrbError;
use crate::format::{Document, Section};
use super::{DocumentImporter, ImportResult, RawChunk};

pub struct PdfImporter;

impl DocumentImporter for PdfImporter {
    fn import(&self, path: &Path) -> Result<ImportResult, OrbError> {
        let pages = pdf_extract::extract_text_by_pages(path)
            .map_err(|e| OrbError::DocumentProcessing(format!("pdf-extract failed: {e}")))?;

        // Check if extraction yielded meaningful text
        let total_text: String = pages.iter().map(|p| p.as_str()).collect();
        if total_text.trim().len() < 100 {
            // Try lopdf as a signal — if it can open the file, it's a valid PDF but likely scanned
            match lopdf::Document::load(path) {
                Ok(_) => {
                    return Err(OrbError::DocumentProcessing(
                        "This PDF appears to be scanned or image-only; OCR is not supported yet."
                            .to_string(),
                    ))
                }
                Err(e) => {
                    return Err(OrbError::DocumentProcessing(format!(
                        "Failed to parse PDF: {e}"
                    )))
                }
            }
        }

        let title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled")
            .to_string();

        let page_count = pages.len();
        let mut sections = Vec::new();
        let mut raw_chunks = Vec::new();

        // Group pages into sections of ~10 pages
        let section_size = 10usize;
        for (section_idx, page_group) in pages.chunks(section_size).enumerate() {
            let page_start = (section_idx * section_size + 1) as u32;
            let page_end = (section_idx * section_size + page_group.len()) as u32;
            let section_id = section_idx as u32;

            // Try to detect a section title from the first page of the group
            let first_page = &page_group[0];
            let section_title = first_page
                .lines()
                .find(|l| {
                    let trimmed = l.trim();
                    !trimmed.is_empty() && trimmed.len() < 80
                })
                .map(|l| l.trim().to_string())
                .unwrap_or_else(|| format!("Pages {page_start}–{page_end}"));

            sections.push(Section {
                id: section_id,
                document_id: 0,
                title: section_title,
                page_start: Some(page_start),
                page_end: Some(page_end),
            });

            // Each page in the group becomes a raw chunk
            for (page_offset, page_text) in page_group.iter().enumerate() {
                let page_num = (section_idx * section_size + page_offset + 1) as u32;
                if !page_text.trim().is_empty() {
                    raw_chunks.push(RawChunk {
                        text: page_text.trim().to_string(),
                        page: Some(page_num),
                        section_id: Some(section_id),
                    });
                }
            }
        }

        let document = Document {
            id: 0,
            title,
            source_path: path.to_string_lossy().to_string(),
            page_count: Some(page_count),
            sections,
        };

        Ok(ImportResult { document, raw_chunks })
    }

    fn supported_extensions(&self) -> &[&str] {
        &["pdf"]
    }
}
