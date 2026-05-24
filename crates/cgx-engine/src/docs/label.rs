//! Replace cgx's dominant-node-name community labels with labels derived from
//! the common path prefix of nodes in each community. Much more readable.
//!
//! e.g. nodes spanning `crates/cgx-engine/src/parsers/*.rs` → label `cgx-engine/parsers`.

use std::collections::HashMap;

use crate::graph::Node;

/// Build a per-community label keyed by community id.
///
/// Algorithm:
/// 1. Collect all node paths per community.
/// 2. Strip a common leading directory prefix (longest common one ending at `/`).
/// 3. If the prefix is informative (>=2 segments or contains a recognisable dir),
///    use it. Otherwise fall back to the cgx-supplied label, then `community-N`.
pub fn build_community_labels(
    all_nodes: &[Node],
    cgx_labels: &HashMap<i64, String>,
) -> HashMap<i64, String> {
    let mut by_community: HashMap<i64, Vec<&str>> = HashMap::new();
    for n in all_nodes {
        if n.path.is_empty() {
            continue;
        }
        by_community
            .entry(n.community)
            .or_default()
            .push(n.path.as_str());
    }

    let mut out: HashMap<i64, String> = HashMap::new();
    for (community, paths) in by_community.iter() {
        let label = derive_label(
            paths,
            cgx_labels.get(community).map(|s| s.as_str()),
            *community,
        );
        out.insert(*community, label);
    }
    out
}

fn derive_label(paths: &[&str], fallback: Option<&str>, community: i64) -> String {
    if paths.is_empty() {
        return fallback_label(fallback, community);
    }

    // Single-file communities: use the file basename without extension.
    let unique: std::collections::HashSet<&&str> = paths.iter().collect();
    if unique.len() == 1 {
        let path = paths[0];
        return path_to_label(path);
    }

    let prefix = common_dir_prefix(paths);
    if !prefix.is_empty() && segment_count(&prefix) >= 2 {
        return condense(&prefix);
    }
    if !prefix.is_empty() {
        return condense(&prefix);
    }
    fallback_label(fallback, community)
}

fn fallback_label(fallback: Option<&str>, community: i64) -> String {
    if let Some(s) = fallback {
        let trimmed = s.trim();
        if !trimmed.is_empty() && !trimmed.starts_with("File:") && !trimmed.starts_with("Function:")
        {
            return trimmed.to_string();
        }
    }
    format!("community-{}", community)
}

fn path_to_label(path: &str) -> String {
    // Drop file extension, dasherise.
    let last_segment = path.rsplit('/').next().unwrap_or(path);
    let stem = last_segment
        .rsplit_once('.')
        .map(|(a, _)| a)
        .unwrap_or(last_segment);
    let parent = path.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
    let parent_tail = parent.rsplit('/').next().unwrap_or("");
    if parent_tail.is_empty() || parent_tail == "src" || parent_tail == "." {
        stem.to_string()
    } else {
        format!("{}/{}", parent_tail, stem)
    }
}

fn common_dir_prefix(paths: &[&str]) -> String {
    if paths.is_empty() {
        return String::new();
    }
    let first_dirs: Vec<&str> = path_dirs(paths[0]);
    let mut common: Vec<&str> = first_dirs;
    for p in paths.iter().skip(1) {
        let dirs = path_dirs(p);
        let take = common
            .iter()
            .zip(dirs.iter())
            .take_while(|(a, b)| a == b)
            .count();
        common.truncate(take);
        if common.is_empty() {
            break;
        }
    }
    common.join("/")
}

fn path_dirs(path: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = path.split('/').collect();
    parts.pop(); // drop filename
    parts
}

fn segment_count(prefix: &str) -> usize {
    prefix.split('/').filter(|s| !s.is_empty()).count()
}

/// Condense a long prefix by trimming common boilerplate roots.
fn condense(prefix: &str) -> String {
    let trimmed = prefix
        .trim_start_matches("crates/")
        .trim_start_matches("packages/")
        .trim_start_matches("src/")
        .trim_start_matches("./");
    if trimmed.is_empty() {
        prefix.trim_start_matches('/').to_string()
    } else {
        trimmed.to_string()
    }
}
