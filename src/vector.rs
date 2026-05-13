use std::cmp::Ordering;

pub type VectorId = String;

#[derive(Clone, Debug, PartialEq)]
pub struct VectorMatch {
    pub id: VectorId,
    pub score: f32,
}

pub trait VectorIndex {
    fn add(&mut self, id: VectorId, embedding: Vec<f32>);
    fn search(&self, query: &[f32], top_k: usize) -> Vec<VectorMatch>;
}

pub struct InMemoryVectorIndex {
    items: Vec<(VectorId, Vec<f32>)>,
}

impl InMemoryVectorIndex {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }
}

impl VectorIndex for InMemoryVectorIndex {
    fn add(&mut self, id: VectorId, embedding: Vec<f32>) {
        if let Some(slot) = self
            .items
            .iter_mut()
            .find(|(existing_id, _)| existing_id == &id)
        {
            slot.1 = embedding;
            return;
        }
        self.items.push((id, embedding));
    }

    fn search(&self, query: &[f32], top_k: usize) -> Vec<VectorMatch> {
        if top_k == 0 || self.items.is_empty() {
            return Vec::new();
        }
        let mut scored: Vec<VectorMatch> = self
            .items
            .iter()
            .map(|(id, vec)| VectorMatch {
                id: id.clone(),
                score: cosine_similarity(query, vec),
            })
            .collect();
        scored.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(Ordering::Equal)
        });
        scored.truncate(top_k);
        scored
    }
}

pub fn embed_text(text: &str, dim: usize) -> Vec<f32> {
    assert!(dim > 0, "embedding dim must be positive");
    let mut vec = vec![0f32; dim];
    let mut tokens = 0usize;
    for token in text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|segment| !segment.is_empty())
    {
        let lower = token.to_ascii_lowercase();
        let bucket = (fnv1a_64(lower.as_bytes()) as usize) % dim;
        vec[bucket] += 1.0;
        tokens += 1;
    }
    if tokens > 0 {
        l2_normalize(&mut vec);
    }
    vec
}

pub fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    if left.len() != right.len() {
        return 0.0;
    }
    let mut dot = 0f32;
    let mut left_sq = 0f32;
    let mut right_sq = 0f32;
    for (a, b) in left.iter().zip(right.iter()) {
        dot += a * b;
        left_sq += a * a;
        right_sq += b * b;
    }
    let denom = left_sq.sqrt() * right_sq.sqrt();
    if denom == 0.0 { 0.0 } else { dot / denom }
}

fn l2_normalize(values: &mut [f32]) {
    let norm: f32 = values.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in values.iter_mut() {
            *value /= norm;
        }
    }
}

fn fnv1a_64(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::{InMemoryVectorIndex, VectorIndex, cosine_similarity, embed_text};

    #[test]
    fn similar_text_ranks_above_unrelated_text() {
        let mut index = InMemoryVectorIndex::new();
        index.add(
            "tokio".to_string(),
            embed_text("rust async runtime tokio futures", 256),
        );
        index.add(
            "docs".to_string(),
            embed_text("write docs for the parser changes", 256),
        );
        index.add(
            "rename".to_string(),
            embed_text("rename a variable in the helper", 256),
        );

        let query = embed_text("tokio runtime futures rust", 256);
        let hits = index.search(&query, 3);
        assert_eq!(hits.first().expect("at least one hit").id, "tokio");
        assert!(hits[0].score > hits[1].score);
    }

    #[test]
    fn add_replaces_existing_id() {
        let mut index = InMemoryVectorIndex::new();
        index.add("x".to_string(), embed_text("first", 64));
        index.add(
            "x".to_string(),
            embed_text("second different content entirely", 64),
        );
        let hits = index.search(&embed_text("second", 64), 10);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "x");
    }

    #[test]
    fn empty_query_or_empty_index_is_empty() {
        let empty = InMemoryVectorIndex::new();
        assert!(empty.search(&[0.0; 8], 5).is_empty());

        let mut populated = InMemoryVectorIndex::new();
        populated.add("a".to_string(), embed_text("anything", 16));
        assert!(populated.search(&[0.0; 16], 0).is_empty());
    }

    #[test]
    fn cosine_handles_edge_cases() {
        assert_eq!(cosine_similarity(&[0.0; 4], &[0.0; 4]), 0.0);
        assert_eq!(cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]), 0.0);
        assert!((cosine_similarity(&[1.0, 1.0], &[1.0, 1.0]) - 1.0).abs() < 1e-6);
        assert_eq!(cosine_similarity(&[1.0, 0.0], &[1.0]), 0.0);
    }
}
