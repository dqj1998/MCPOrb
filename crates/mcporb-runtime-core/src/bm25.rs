use std::collections::HashMap;

use unicode_segmentation::UnicodeSegmentation;

use crate::format::{Bm25Index, Chunk};

const K1: f32 = 1.5;
const B: f32 = 0.75;

pub fn tokenize(text: &str) -> Vec<String> {
    text.unicode_words()
        .map(|word| word.to_lowercase())
        .filter(|word| word.chars().any(|ch| ch.is_alphanumeric()))
        .collect()
}

pub fn build_index(chunks: &[Chunk]) -> Bm25Index {
    let mut vocab: HashMap<String, u32> = HashMap::new();
    let mut next_term_id: u32 = 0;
    let mut doc_tfs: Vec<HashMap<u32, u32>> = Vec::with_capacity(chunks.len());
    let mut doc_lengths: Vec<usize> = Vec::with_capacity(chunks.len());

    for chunk in chunks {
        let tokens = tokenize(&chunk.text);
        doc_lengths.push(tokens.len());

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

    let mut postings: Vec<Vec<(u32, f32)>> = vec![Vec::new(); vocab.len()];
    for (chunk_id, tf_map) in doc_tfs.iter().enumerate() {
        for (&term_id, &tf) in tf_map {
            postings[term_id as usize].push((chunk_id as u32, tf as f32));
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
        let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();

        for &(chunk_id, tf) in postings {
            let dl = index.doc_lengths[chunk_id as usize] as f32;
            let avg_dl = index.avg_doc_len;
            let tf_norm = (tf * (K1 + 1.0)) / (tf + K1 * (1.0 - B + B * dl / avg_dl.max(1.0)));
            *scores.entry(chunk_id).or_insert(0.0) += idf * tf_norm;
        }
    }

    let mut ranked: Vec<(u32, f32)> = scores.into_iter().collect();
    ranked.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    ranked.truncate(top_k);
    ranked
}
