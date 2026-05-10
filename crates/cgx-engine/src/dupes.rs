use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::graph::Node;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CloneKind {
    Exact,
    Near,
}

impl CloneKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            CloneKind::Exact => "exact",
            CloneKind::Near => "near",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClonePair {
    pub node_a_id: String,
    pub node_b_id: String,
    pub node_a_name: String,
    pub node_b_name: String,
    pub node_a_path: String,
    pub node_b_path: String,
    pub node_a_line: u32,
    pub node_b_line: u32,
    pub similarity: f64,
    pub kind: CloneKind,
}

/// Normalize a function body for comparison:
/// - Strip comments (// and /* */ style)
/// - Normalize whitespace (collapse to single space)
/// - Replace string literals with "S"
/// - Replace number literals with "N"
pub fn normalize_body(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Skip line comments
        if i + 1 < chars.len() && chars[i] == '/' && chars[i + 1] == '/' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        // Skip block comments
        if i + 1 < chars.len() && chars[i] == '/' && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i += 2; // skip */
            continue;
        }
        // Replace string literals with S
        if chars[i] == '"' || chars[i] == '\'' || chars[i] == '`' {
            let quote = chars[i];
            i += 1;
            while i < chars.len() {
                if chars[i] == '\\' {
                    i += 2; // skip escaped char
                    continue;
                }
                if chars[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            result.push('S');
            continue;
        }
        // Replace number literals with N
        if chars[i].is_ascii_digit() || (chars[i] == '.' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit()) {
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == '_' || chars[i] == 'x' || chars[i] == 'o' || chars[i] == 'b') {
                i += 1;
            }
            result.push('N');
            continue;
        }
        // Normalize whitespace
        if chars[i].is_whitespace() {
            result.push(' ');
            i += 1;
            // Skip additional whitespace
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            continue;
        }
        result.push(chars[i]);
        i += 1;
    }

    result.trim().to_string()
}

/// FNV hash of normalized body using DefaultHasher
pub fn fingerprint(body_text: &str) -> u64 {
    let normalized = normalize_body(body_text);
    let mut hasher = DefaultHasher::new();
    normalized.hash(&mut hasher);
    hasher.finish()
}

/// Jaccard similarity on character 4-grams
pub fn text_similarity(a: &str, b: &str) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let ngrams_a: std::collections::HashSet<_> = a
        .as_bytes()
        .windows(4)
        .collect();
    let ngrams_b: std::collections::HashSet<_> = b
        .as_bytes()
        .windows(4)
        .collect();

    if ngrams_a.is_empty() || ngrams_b.is_empty() {
        return 0.0;
    }

    let intersection = ngrams_a.intersection(&ngrams_b).count();
    let union = ngrams_a.union(&ngrams_b).count();

    if union == 0 {
        return 0.0;
    }

    intersection as f64 / union as f64
}

/// Detect clone pairs among the given nodes.
/// Reads source files, extracts function bodies by line range,
/// fingerprints them, finds exact and near matches.
pub fn detect_clones(
    nodes: &[Node],
    repo_path: &Path,
    threshold: f64,
) -> anyhow::Result<Vec<ClonePair>> {
    // Only consider function nodes with a real path and line range
    let fn_nodes: Vec<&Node> = nodes
        .iter()
        .filter(|n| n.kind == "Function" && !n.path.is_empty() && n.line_end >= n.line_start)
        .collect();

    if fn_nodes.len() < 2 {
        return Ok(Vec::new());
    }

    // Load file contents (cached by path)
    let mut file_cache: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

    let mut bodies: Vec<(usize, String)> = Vec::new(); // (node index, normalized body)

    for (idx, node) in fn_nodes.iter().enumerate() {
        let file_lines = file_cache.entry(node.path.clone()).or_insert_with(|| {
            let full_path = repo_path.join(&node.path);
            std::fs::read_to_string(&full_path)
                .unwrap_or_default()
                .lines()
                .map(|l| l.to_string())
                .collect()
        });

        let start = (node.line_start as usize).saturating_sub(1);
        let end = (node.line_end as usize).min(file_lines.len());
        if start >= end {
            bodies.push((idx, String::new()));
            continue;
        }

        let body = file_lines[start..end].join("\n");
        let normalized = normalize_body(&body);
        bodies.push((idx, normalized));
    }

    // Skip functions with very short bodies (less than 4 chars normalized)
    let valid_bodies: Vec<(usize, &str)> = bodies
        .iter()
        .filter(|(_, b)| b.len() >= 4)
        .map(|(idx, b)| (*idx, b.as_str()))
        .collect();

    // Build fingerprint map for exact matches
    let mut fp_map: std::collections::HashMap<u64, Vec<usize>> =
        std::collections::HashMap::new();
    for (idx, body) in &valid_bodies {
        let mut hasher = DefaultHasher::new();
        body.hash(&mut hasher);
        let fp = hasher.finish();
        fp_map.entry(fp).or_default().push(*idx);
    }

    let mut pairs: Vec<ClonePair> = Vec::new();
    let mut seen: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();

    // Exact matches
    for group in fp_map.values() {
        if group.len() < 2 {
            continue;
        }
        for i in 0..group.len() {
            for j in (i + 1)..group.len() {
                let idx_a = group[i];
                let idx_b = group[j];
                let key = (idx_a.min(idx_b), idx_a.max(idx_b));
                if seen.insert(key) {
                    let node_a = fn_nodes[idx_a];
                    let node_b = fn_nodes[idx_b];
                    pairs.push(ClonePair {
                        node_a_id: node_a.id.clone(),
                        node_b_id: node_b.id.clone(),
                        node_a_name: node_a.name.clone(),
                        node_b_name: node_b.name.clone(),
                        node_a_path: node_a.path.clone(),
                        node_b_path: node_b.path.clone(),
                        node_a_line: node_a.line_start,
                        node_b_line: node_b.line_start,
                        similarity: 1.0,
                        kind: CloneKind::Exact,
                    });
                }
            }
        }
    }

    // Near matches — only do pairwise comparison if count is manageable
    if valid_bodies.len() <= 500 {
        for i in 0..valid_bodies.len() {
            for j in (i + 1)..valid_bodies.len() {
                let idx_a = valid_bodies[i].0;
                let idx_b = valid_bodies[j].0;
                let key = (idx_a.min(idx_b), idx_a.max(idx_b));
                if seen.contains(&key) {
                    continue; // already an exact match
                }
                let body_a = valid_bodies[i].1;
                let body_b = valid_bodies[j].1;
                let sim = text_similarity(body_a, body_b);
                if sim >= threshold {
                    seen.insert(key);
                    let node_a = fn_nodes[idx_a];
                    let node_b = fn_nodes[idx_b];
                    pairs.push(ClonePair {
                        node_a_id: node_a.id.clone(),
                        node_b_id: node_b.id.clone(),
                        node_a_name: node_a.name.clone(),
                        node_b_name: node_b.name.clone(),
                        node_a_path: node_a.path.clone(),
                        node_b_path: node_b.path.clone(),
                        node_a_line: node_a.line_start,
                        node_b_line: node_b.line_start,
                        similarity: sim,
                        kind: CloneKind::Near,
                    });
                }
            }
        }
    }

    // Sort by similarity descending
    pairs.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(pairs)
}
