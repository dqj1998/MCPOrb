use std::collections::{HashMap, HashSet};

use crate::format::{Chunk, TrigramIndex};

pub fn build_index(chunks: &[Chunk], min_df: usize) -> TrigramIndex {
    let chunk_trigrams: Vec<HashSet<String>> = chunks
        .iter()
        .map(|chunk| extract_trigrams(&chunk.text))
        .collect();

    let mut df_counts: HashMap<String, usize> = HashMap::new();
    for trigrams in &chunk_trigrams {
        for trigram in trigrams {
            *df_counts.entry(trigram.clone()).or_insert(0) += 1;
        }
    }

    let mut vocab: HashMap<String, u32> = HashMap::new();
    let mut next_id = 0u32;
    for (trigram, df) in df_counts {
        if df >= min_df {
            vocab.insert(trigram, next_id);
            next_id += 1;
        }
    }

    let mut postings: Vec<Vec<u32>> = vec![Vec::new(); vocab.len()];
    let mut trigram_counts = Vec::with_capacity(chunk_trigrams.len());

    for (chunk_id, trigrams) in chunk_trigrams.iter().enumerate() {
        let mut count = 0u32;
        for trigram in trigrams {
            if let Some(&term_id) = vocab.get(trigram) {
                postings[term_id as usize].push(chunk_id as u32);
                count += 1;
            }
        }
        trigram_counts.push(count);
    }

    for posting in &mut postings {
        posting.sort_unstable();
    }

    TrigramIndex {
        doc_count: chunks.len(),
        vocab,
        postings,
        trigram_counts,
    }
}

pub fn search(index: &TrigramIndex, query: &str, top_k: usize) -> Vec<(u32, f32)> {
    let query_trigrams = extract_trigrams(query);
    let query_count = query_trigrams.len() as f32;
    if query_count == 0.0 {
        return vec![];
    }

    let mut intersections: HashMap<u32, u32> = HashMap::new();
    for trigram in &query_trigrams {
        if let Some(&term_id) = index.vocab.get(trigram) {
            for &chunk_id in &index.postings[term_id as usize] {
                *intersections.entry(chunk_id).or_insert(0) += 1;
            }
        }
    }

    let mut results: Vec<(u32, f32)> = intersections
        .into_iter()
        .map(|(chunk_id, intersection)| {
            let doc_count = index.trigram_counts[chunk_id as usize] as f32;
            let union = query_count + doc_count - intersection as f32;
            let score = if union > 0.0 {
                intersection as f32 / union
            } else {
                0.0
            };
            (chunk_id, score)
        })
        .collect();

    results.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(top_k);
    results
}

fn extract_trigrams(text: &str) -> HashSet<String> {
    let chars: Vec<char> = text.to_lowercase().chars().collect();
    if chars.len() < 3 {
        return HashSet::new();
    }

    chars
        .windows(3)
        .map(|window| window.iter().collect::<String>())
        .collect()
}
