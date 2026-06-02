use anyhow::{anyhow, Result};

use crate::format::FlatVectorIndex;

#[cfg(feature = "hnsw")]
use instant_distance::{Builder as HnswBuilder, HnswMap, Point, Search};

#[derive(Clone, Debug)]
pub struct DensePoint {
    values: Vec<f32>,
}

impl DensePoint {
    fn normalized(values: &[f32]) -> Self {
        Self {
            values: normalize_copy(values),
        }
    }
}

#[cfg(feature = "hnsw")]
impl Point for DensePoint {
    fn distance(&self, other: &Self) -> f32 {
        cosine_distance(&self.values, &other.values)
    }
}

#[cfg(feature = "hnsw")]
pub type DenseHnswMap = HnswMap<DensePoint, u32>;

pub fn validate_query_vector(index: &FlatVectorIndex, query_vector: &[f32]) -> Result<()> {
    if query_vector.len() != index.dim {
        return Err(anyhow!(
            "query_vector dimension mismatch: expected {}, got {}",
            index.dim,
            query_vector.len()
        ));
    }

    Ok(())
}

pub fn search(index: &FlatVectorIndex, query_vector: &[f32], top_k: usize) -> Vec<(u32, f32)> {
    let query = normalize_copy(query_vector);
    let mut hits = index
        .vectors
        .chunks_exact(index.dim)
        .enumerate()
        .map(|(chunk_id, vector)| (chunk_id as u32, dot(&query, vector)))
        .collect::<Vec<_>>();

    hits.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.0.cmp(&right.0))
    });
    hits.truncate(top_k);
    hits
}

#[cfg(feature = "hnsw")]
pub fn build_hnsw(
    index: &FlatVectorIndex,
    ef_construction: usize,
    ef_search: usize,
) -> DenseHnswMap {
    let points = index
        .vectors
        .chunks_exact(index.dim)
        .map(DensePoint::normalized)
        .collect::<Vec<_>>();
    let values = (0..index.chunk_count as u32).collect::<Vec<_>>();

    HnswBuilder::default()
        .ef_construction(ef_construction)
        .ef_search(ef_search)
        .build(points, values)
}

#[cfg(feature = "hnsw")]
pub fn search_hnsw(map: &DenseHnswMap, query_vector: &[f32], top_k: usize) -> Vec<(u32, f32)> {
    let query = DensePoint::normalized(query_vector);
    let mut search = Search::default();
    map.search(&query, &mut search)
        .take(top_k)
        .map(|item| (*item.value, (1.0 - item.distance).clamp(-1.0, 1.0)))
        .collect()
}

fn normalize_copy(values: &[f32]) -> Vec<f32> {
    let mut normalized = values.to_vec();
    let norm = normalized
        .iter()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    if norm > 0.0 {
        for value in &mut normalized {
            *value /= norm;
        }
    }
    normalized
}

fn dot(left: &[f32], right: &[f32]) -> f32 {
    left.iter().zip(right).map(|(l, r)| l * r).sum()
}

fn cosine_distance(left: &[f32], right: &[f32]) -> f32 {
    (1.0 - dot(left, right)).clamp(0.0, 2.0)
}
