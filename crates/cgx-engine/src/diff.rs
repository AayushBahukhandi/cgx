use std::path::Path;

use anyhow::Context;

use crate::parser::{EdgeDef, NodeDef, ParserRegistry};
use crate::walker::{Language, SourceFile};

#[derive(Debug, Clone)]
pub struct GraphSnapshot {
    pub nodes: Vec<NodeDef>,
    pub edges: Vec<EdgeDef>,
    pub commit: String,
}

#[derive(Debug, Clone)]
pub struct GraphDiff {
    pub added_nodes: Vec<NodeDef>,
    pub removed_nodes: Vec<NodeDef>,
    pub added_edges: Vec<EdgeDef>,
    pub removed_edges: Vec<EdgeDef>,
    pub modified_nodes: Vec<(NodeDef, NodeDef)>,
}

/// Take a graph snapshot by parsing the source tree at a specific git commit.
pub fn snapshot_at_commit(repo_path: &Path, commit_spec: &str) -> anyhow::Result<GraphSnapshot> {
    let repo = git2::Repository::open(repo_path).context("Failed to open git repository")?;

    let obj = repo
        .revparse_single(commit_spec)
        .context(format!("Invalid commit reference: {}", commit_spec))?;
    let commit = obj
        .peel_to_commit()
        .context("Reference does not resolve to a commit")?;
    let tree = commit.tree()?;
    let commit_sha = commit.id().to_string();

    let mut files: Vec<SourceFile> = Vec::new();
    walk_tree(&repo, &tree, Path::new(""), &mut files)?;

    let registry = ParserRegistry::new();
    let results = registry.parse_all(&files);

    let mut nodes: Vec<NodeDef> = Vec::new();
    let mut edges: Vec<EdgeDef> = Vec::new();

    for result in &results {
        nodes.extend(result.nodes.clone());
        edges.extend(result.edges.clone());
    }

    // Add file nodes
    let lang_map = crate::resolver::build_language_map(&nodes);
    let file_paths: std::collections::HashSet<String> =
        files.iter().map(|f| f.relative_path.clone()).collect();
    let file_nodes = crate::resolver::create_file_nodes(&file_paths, &lang_map);
    nodes.extend(file_nodes);

    Ok(GraphSnapshot {
        nodes,
        edges,
        commit: commit_sha,
    })
}

fn walk_tree(
    repo: &git2::Repository,
    tree: &git2::Tree,
    prefix: &Path,
    files: &mut Vec<SourceFile>,
) -> anyhow::Result<()> {
    for entry in tree.iter() {
        let name = entry.name().unwrap_or("unknown");
        let path = prefix.join(name);

        match entry.kind() {
            Some(git2::ObjectType::Blob) => {
                let relative = path.to_string_lossy().to_string();
                if let Some(lang) = detect_language(&relative) {
                    let blob = entry.to_object(repo)?;
                    let blob = blob.peel_to_blob()?;
                    if let Ok(content) = std::str::from_utf8(blob.content()) {
                        if content.len() < 2_000_000 && !is_binary(content) {
                            files.push(SourceFile {
                                path: repo.workdir().unwrap_or(Path::new(".")).join(&path),
                                relative_path: relative,
                                language: lang,
                                content: content.to_string(),
                                size_bytes: content.len() as u64,
                            });
                        }
                    }
                }
            }
            Some(git2::ObjectType::Tree) => {
                let subtree = entry.to_object(repo)?.peel_to_tree()?;
                walk_tree(repo, &subtree, &path, files)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn detect_language(path: &str) -> Option<Language> {
    let lower = path.to_lowercase();
    if lower.ends_with(".ts") || lower.ends_with(".tsx") {
        Some(Language::TypeScript)
    } else if lower.ends_with(".js") || lower.ends_with(".jsx") || lower.ends_with(".mjs") {
        Some(Language::JavaScript)
    } else if lower.ends_with(".py") {
        Some(Language::Python)
    } else if lower.ends_with(".rs") {
        Some(Language::Rust)
    } else {
        None
    }
}

fn is_binary(content: &str) -> bool {
    content.as_bytes().iter().take(8192).any(|&b| b == 0)
}

/// Compute the diff between two graph snapshots.
pub fn diff_graphs(before: &GraphSnapshot, after: &GraphSnapshot) -> GraphDiff {
    let before_nodes: std::collections::HashMap<&str, &NodeDef> =
        before.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let after_nodes: std::collections::HashMap<&str, &NodeDef> =
        after.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    let mut added_nodes = Vec::new();
    let mut removed_nodes = Vec::new();
    let mut modified_nodes = Vec::new();

    for (id, node) in &after_nodes {
        if let Some(before) = before_nodes.get(id) {
            // Check if modified
            if before.name != node.name
                || before.path != node.path
                || before.line_start != node.line_start
                || before.line_end != node.line_end
                || before.kind != node.kind
            {
                modified_nodes.push(((**before).clone(), (**node).clone()));
            }
        } else {
            added_nodes.push((**node).clone());
        }
    }

    for (id, node) in &before_nodes {
        if !after_nodes.contains_key(id) {
            removed_nodes.push((**node).clone());
        }
    }

    let mut added_edges = Vec::new();
    let mut removed_edges = Vec::new();

    let before_edge_ids: std::collections::HashSet<String> =
        before.edges.iter().map(id_from_edge).collect();
    let after_edge_ids: std::collections::HashSet<String> =
        after.edges.iter().map(id_from_edge).collect();

    for edge in &after.edges {
        let id = id_from_edge(edge);
        if !before_edge_ids.contains(id.as_str()) {
            added_edges.push(edge.clone());
        }
    }

    for edge in &before.edges {
        let id = id_from_edge(edge);
        if !after_edge_ids.contains(id.as_str()) {
            removed_edges.push(edge.clone());
        }
    }

    GraphDiff {
        added_nodes,
        removed_nodes,
        added_edges,
        removed_edges,
        modified_nodes,
    }
}

fn id_from_edge(e: &EdgeDef) -> String {
    format!("{}|{}|{}", e.src, e.kind.as_str(), e.dst)
}

/// Find files changed in the last N days and compute impact.
pub fn compute_impact(repo_path: &Path, since_days: u32) -> anyhow::Result<ImpactReport> {
    let repo = git2::Repository::open(repo_path).context("Failed to open git repository")?;

    // Get files changed since N days ago
    let cutoff = chrono::Utc::now() - chrono::Duration::days(since_days as i64);
    let cutoff_epoch = cutoff.timestamp();

    let mut changed_files: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;

    for oid in revwalk {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;
        let commit_time = commit.time().seconds();

        if commit_time < cutoff_epoch {
            break;
        }

        if commit.parent_count() == 0 {
            let tree = commit.tree()?;
            let diff = repo.diff_tree_to_tree(None, Some(&tree), None)?;
            diff.foreach(
                &mut |delta, _| {
                    if let Some(path) = delta.new_file().path() {
                        changed_files.insert(path.to_string_lossy().to_string());
                    }
                    true
                },
                None,
                None,
                None,
            )?;
        } else {
            for i in 0..commit.parent_count() {
                let parent = commit.parent(i)?;
                let parent_tree = parent.tree()?;
                let tree = commit.tree()?;
                let diff = repo.diff_tree_to_tree(Some(&parent_tree), Some(&tree), None)?;
                diff.foreach(
                    &mut |delta, _| {
                        if let Some(path) = delta.new_file().path() {
                            changed_files.insert(path.to_string_lossy().to_string());
                        }
                        true
                    },
                    None,
                    None,
                    None,
                )?;
            }
        }
    }

    // Load the graph from DuckDB
    let db = crate::GraphDb::open(repo_path)?;
    let all_nodes = db.get_all_nodes()?;
    let all_edges = db.get_all_edges()?;

    // Find nodes in changed files
    let changed_nodes: Vec<&crate::Node> = all_nodes
        .iter()
        .filter(|n| changed_files.contains(&n.path))
        .collect();

    // Build reverse adjacency: what depends on what
    let mut rev_adj: std::collections::HashMap<&str, Vec<&str>> = std::collections::HashMap::new();
    for e in &all_edges {
        rev_adj
            .entry(e.dst.as_str())
            .or_default()
            .push(e.src.as_str());
    }

    let mut downstream = std::collections::HashSet::new();
    let mut dq: Vec<&str> = changed_nodes.iter().map(|n| n.id.as_str()).collect();
    let mut seen = std::collections::HashSet::new();

    while let Some(current) = dq.pop() {
        if let Some(dependents) = rev_adj.get(current) {
            for &dep in dependents {
                if seen.insert(dep) {
                    downstream.insert(dep);
                    dq.push(dep);
                }
            }
        }
    }

    // Count affected
    let total_affected = downstream.len() + changed_nodes.len();

    let node_map: std::collections::HashMap<&str, &crate::Node> =
        all_nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    let affected_nodes: Vec<&crate::Node> = downstream
        .iter()
        .filter_map(|id| node_map.get(id))
        .copied()
        .collect();

    Ok(ImpactReport {
        changed_files: changed_files.into_iter().collect(),
        changed_nodes: changed_nodes.into_iter().cloned().collect(),
        impacted_nodes: affected_nodes.into_iter().cloned().collect(),
        total_impacted: total_affected,
    })
}

#[derive(Debug, Clone)]
pub struct ImpactReport {
    pub changed_files: Vec<String>,
    pub changed_nodes: Vec<crate::Node>,
    pub impacted_nodes: Vec<crate::Node>,
    pub total_impacted: usize,
}
