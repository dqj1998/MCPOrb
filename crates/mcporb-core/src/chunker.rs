use crate::format::Chunk;
use crate::importer::RawChunk;

pub struct ChunkerConfig {
    pub chunk_size: usize,
    pub overlap: usize,
}

impl Default for ChunkerConfig {
    fn default() -> Self {
        Self {
            chunk_size: 800,
            overlap: 100,
        }
    }
}

pub fn chunk_raw(
    raw_chunks: Vec<RawChunk>,
    document_id: u32,
    config: &ChunkerConfig,
) -> Vec<Chunk> {
    let mut result = Vec::new();
    let mut chunk_id: u32 = 0;

    for raw in raw_chunks {
        let paragraphs: Vec<&str> = raw.text.split("\n\n").collect();
        let mut current = String::new();

        for para in &paragraphs {
            let para = para.trim();
            if para.is_empty() {
                continue;
            }

            if current.len() + para.len() + 2 > config.chunk_size && !current.is_empty() {
                // Emit current chunk
                let text = current.trim().to_string();
                let token_count = text.split_whitespace().count();
                result.push(Chunk {
                    id: chunk_id,
                    document_id,
                    section_id: raw.section_id,
                    page: raw.page,
                    text,
                    token_count,
                });
                chunk_id += 1;

                // Overlap: keep last `overlap` chars
                let overlap_start = current.len().saturating_sub(config.overlap);
                current = current[overlap_start..].to_string();
            }

            if !current.is_empty() {
                current.push_str("\n\n");
            }
            current.push_str(para);
        }

        // Emit remaining
        if !current.trim().is_empty() {
            let text = current.trim().to_string();
            let token_count = text.split_whitespace().count();
            result.push(Chunk {
                id: chunk_id,
                document_id,
                section_id: raw.section_id,
                page: raw.page,
                text,
                token_count,
            });
            chunk_id += 1;
        }
    }

    result
}
