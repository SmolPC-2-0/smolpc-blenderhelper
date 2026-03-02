use crate::rag::types::RagChunk;
use std::collections::HashSet;

pub fn keyword_top_k(chunks: &[RagChunk], query: &str, top_k: usize) -> Vec<(usize, f32)> {
    if chunks.is_empty() || query.trim().is_empty() || top_k == 0 {
        return Vec::new();
    }

    let query_terms = tokenize(query);
    if query_terms.is_empty() {
        return Vec::new();
    }

    let mut scored: Vec<(usize, f32)> = chunks
        .iter()
        .enumerate()
        .map(|(idx, chunk)| {
            let combined = format!("{} {}", chunk.signature, chunk.text);
            let chunk_terms = tokenize(&combined);
            let overlap = query_terms.intersection(&chunk_terms).count() as f32;

            // Normalized overlap score with a small signature bonus.
            let base_score = overlap / query_terms.len() as f32;
            let signature_bonus = if contains_any(&chunk.signature, &query_terms) {
                0.1
            } else {
                0.0
            };

            (idx, (base_score + signature_bonus).min(1.0))
        })
        .collect();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(top_k.min(scored.len()));
    scored
}

fn tokenize(text: &str) -> HashSet<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|token| token.len() > 2)
        .map(ToString::to_string)
        .collect()
}

fn contains_any(text: &str, terms: &HashSet<String>) -> bool {
    let lowered = text.to_lowercase();
    terms.iter().any(|term| lowered.contains(term))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_top_match_for_overlapping_terms() {
        let chunks = vec![
            RagChunk {
                text: "Use bevel modifier to smooth hard edges".to_string(),
                signature: "bpy.types.BevelModifier".to_string(),
                url: "/bpy.types.BevelModifier.html".to_string(),
            },
            RagChunk {
                text: "Material settings control surface appearance".to_string(),
                signature: "bpy.types.Material".to_string(),
                url: "/bpy.types.Material.html".to_string(),
            },
        ];

        let results = keyword_top_k(&chunks, "how do I add a bevel modifier", 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 0);
    }
}
