use std::collections::HashMap;
use unicode_segmentation::UnicodeSegmentation;
use crate::format::{Bm25Index, Chunk};

/// BM25 parameters
const K1: f32 = 1.5;
const B: f32 = 0.75;

/// Tokenize text into lowercase word tokens using unicode word boundaries.
/// Filters out tokens that are purely punctuation or whitespace.
pub fn tokenize(text: &str) -> Vec<String> {
    text.unicode_words()
        .map(|w| w.to_lowercase())
        .filter(|w| w.chars().any(|c| c.is_alphanumeric()))
        .collect()
}

/// Build a BM25 index from a slice of chunks.
pub fn build_index(chunks: &[Chunk]) -> Bm25Index {
    let mut vocab: HashMap<String, u32> = HashMap::new();
    let mut next_term_id: u32 = 0;

    // First pass: build vocabulary and per-document term frequencies
    // doc_tfs[chunk_idx] = HashMap<term_id, tf>
    let mut doc_tfs: Vec<HashMap<u32, u32>> = Vec::with_capacity(chunks.len());
    let mut doc_lengths: Vec<usize> = Vec::with_capacity(chunks.len());

    for chunk in chunks {
        let tokens = tokenize(&chunk.text);
        let len = tokens.len();
        doc_lengths.push(len);

        let mut tf_map: HashMap<u32, u32> = HashMap::new();
        for token in tokens {
            let term_id = *vocab.entry(token).or_insert_with(|| {
                let id = next_term_id;
                next_term_id += 1;
                id
            });
            *tf_map.entry(term_id).or_insert(0) += 1;
        }
        doc_tfs.push(tf_map);
    }

    let doc_count = chunks.len();
    let avg_doc_len = if doc_count == 0 {
        0.0
    } else {
        doc_lengths.iter().sum::<usize>() as f32 / doc_count as f32
    };

    // Build postings lists: postings[term_id] = Vec<(chunk_id, raw_tf)>
    let vocab_size = vocab.len();
    let mut postings: Vec<Vec<(u32, f32)>> = vec![Vec::new(); vocab_size];

    for (chunk_idx, tf_map) in doc_tfs.iter().enumerate() {
        for (&term_id, &tf) in tf_map {
            postings[term_id as usize].push((chunk_idx as u32, tf as f32));
        }
    }

    Bm25Index {
        doc_count,
        avg_doc_len,
        vocab,
        postings,
        doc_lengths,
    }
}

/// Score a query against the index and return top-k chunk IDs sorted by score descending.
pub fn search(index: &Bm25Index, query: &str, top_k: usize) -> Vec<(u32, f32)> {
    if index.doc_count == 0 || query.trim().is_empty() {
        return vec![];
    }

    let query_tokens = tokenize(query);
    let mut scores: HashMap<u32, f32> = HashMap::new();

    for token in &query_tokens {
        let term_id = match index.vocab.get(token) {
            Some(&id) => id,
            None => continue,
        };

        let postings = &index.postings[term_id as usize];
        let df = postings.len() as f32;
        let n = index.doc_count as f32;

        // IDF with smoothing: ln((N - df + 0.5) / (df + 0.5) + 1)
        let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();

        for &(chunk_id, tf) in postings {
            let dl = index.doc_lengths[chunk_id as usize] as f32;
            let avg_dl = index.avg_doc_len;

            // BM25 TF component
            let tf_norm = (tf * (K1 + 1.0))
                / (tf + K1 * (1.0 - B + B * dl / avg_dl.max(1.0)));

            *scores.entry(chunk_id).or_insert(0.0) += idf * tf_norm;
        }
    }

    let mut ranked: Vec<(u32, f32)> = scores.into_iter().collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    ranked.truncate(top_k);
    ranked
}
