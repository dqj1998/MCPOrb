use std::collections::{HashMap, HashSet};

use crate::bm25::tokenize;
use crate::format::{Chunk, TfIdfIndex};

pub fn build_index(chunks: &[Chunk]) -> TfIdfIndex {
    let mut vocab: HashMap<String, u32> = HashMap::new();
    let mut next_term_id: u32 = 0;
    let mut doc_term_freqs: Vec<HashMap<u32, u32>> = Vec::with_capacity(chunks.len());
    let mut doc_freqs: HashMap<u32, u32> = HashMap::new();

    for chunk in chunks {
        let mut term_freqs: HashMap<u32, u32> = HashMap::new();
        let mut seen_terms: HashSet<u32> = HashSet::new();

        for token in tokenize(&chunk.text) {
            let term_id = *vocab.entry(token).or_insert_with(|| {
                let id = next_term_id;
                next_term_id += 1;
                id
            });
            *term_freqs.entry(term_id).or_insert(0) += 1;
            if seen_terms.insert(term_id) {
                *doc_freqs.entry(term_id).or_insert(0) += 1;
            }
        }

        doc_term_freqs.push(term_freqs);
    }

    let doc_count = chunks.len();
    let mut idf = vec![0.0; vocab.len()];
    for &term_id in vocab.values() {
        let df = doc_freqs.get(&term_id).copied().unwrap_or(0) as f32;
        idf[term_id as usize] = (((doc_count as f32) + 1.0) / (df + 1.0)).ln() + 1.0;
    }

    let doc_vectors = doc_term_freqs
        .into_iter()
        .map(|term_freqs| {
            let mut vector: Vec<(u32, f32)> = term_freqs
                .into_iter()
                .map(|(term_id, tf)| {
                    let weight = (1.0 + (tf as f32).ln()) * idf[term_id as usize];
                    (term_id, weight)
                })
                .collect();
            normalize_sparse_vector(&mut vector);
            vector.sort_by_key(|(term_id, _)| *term_id);
            vector
        })
        .collect();

    TfIdfIndex {
        doc_count,
        vocab,
        idf,
        doc_vectors,
    }
}

pub fn search(index: &TfIdfIndex, query: &str, top_k: usize) -> Vec<(u32, f32)> {
    if index.doc_count == 0 || query.trim().is_empty() {
        return vec![];
    }

    let query_vector = build_query_vector(index, query);
    if query_vector.is_empty() {
        return vec![];
    }

    let query_weights: HashMap<u32, f32> = query_vector.iter().copied().collect();
    let mut scores = Vec::new();

    for (chunk_id, doc_vector) in index.doc_vectors.iter().enumerate() {
        let mut score = 0.0;
        for &(term_id, weight) in doc_vector {
            if let Some(query_weight) = query_weights.get(&term_id) {
                score += weight * query_weight;
            }
        }
        if score > 0.0 {
            scores.push((chunk_id as u32, score));
        }
    }

    scores.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scores.truncate(top_k);
    scores
}

fn build_query_vector(index: &TfIdfIndex, query: &str) -> Vec<(u32, f32)> {
    let mut term_freqs: HashMap<u32, u32> = HashMap::new();
    for token in tokenize(query) {
        if let Some(&term_id) = index.vocab.get(&token) {
            *term_freqs.entry(term_id).or_insert(0) += 1;
        }
    }

    let mut vector: Vec<(u32, f32)> = term_freqs
        .into_iter()
        .map(|(term_id, tf)| {
            let weight = (1.0 + (tf as f32).ln()) * index.idf[term_id as usize];
            (term_id, weight)
        })
        .collect();
    normalize_sparse_vector(&mut vector);
    vector
}

fn normalize_sparse_vector(vector: &mut Vec<(u32, f32)>) {
    let norm = vector
        .iter()
        .map(|(_, weight)| weight * weight)
        .sum::<f32>()
        .sqrt();

    if norm > 0.0 {
        for (_, weight) in vector.iter_mut() {
            *weight /= norm;
        }
    }
}
