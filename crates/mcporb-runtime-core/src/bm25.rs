use std::collections::HashMap;

use unicode_segmentation::UnicodeSegmentation;

use crate::format::Bm25Index;

const K1: f32 = 1.5;
const B: f32 = 0.75;

pub fn tokenize(text: &str) -> Vec<String> {
    text.unicode_words()
        .map(|word| word.to_lowercase())
        .filter(|word| word.chars().any(|ch| ch.is_alphanumeric()))
        .collect()
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
