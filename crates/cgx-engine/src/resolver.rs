use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::parser::{EdgeDef, EdgeKind, NodeDef, NodeKind};

/// Returns true if the given relative file path looks like a test file.
pub fn is_test_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.contains("/test/")
        || lower.contains("/tests/")
        || lower.contains("/__tests__/")
        || lower.contains("/spec/")
        || lower.ends_with(".test.ts")
        || lower.ends_with(".spec.ts")
        || lower.ends_with(".test.js")
        || lower.ends_with(".spec.js")
        || lower.ends_with(".test.tsx")
        || lower.ends_with(".spec.tsx")
        || lower.ends_with("_test.py")
        || lower.ends_with("_test.rs")
}

/// Resolve raw parser edges into fully-qualified node IDs.
///
/// Import paths and call targets are matched against the known node set.
/// `CALLS` edges originating from test files are reclassified as `TESTS`
/// when the destination is a production symbol.  Unresolvable edges are
/// kept so later analysis phases can still use them.
pub fn resolve(
    nodes: &[NodeDef],
    edges: &[EdgeDef],
    _repo_root: &Path,
) -> anyhow::Result<Vec<EdgeDef>> {
    let mut resolved_edges: Vec<EdgeDef> = Vec::new();

    // Create a set of all known node IDs for validation
    let node_ids: HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();

    // Build export index: name -> Vec<node_id>
    let mut export_index: HashMap<String, Vec<String>> = HashMap::new();
    for node in nodes {
        export_index
            .entry(node.name.clone())
            .or_default()
            .push(node.id.clone());
    }

    // Build file node set from actual files
    let mut file_paths: HashSet<String> = HashSet::new();
    for node in nodes {
        file_paths.insert(node.path.clone());
    }

    // Create file node IDs we know about
    let known_file_ids: HashSet<String> =
        file_paths.iter().map(|p| format!("file:{}", p)).collect();

    // Build a set of test-file paths for CALLS→TESTS reclassification
    let test_paths: HashSet<&str> = nodes
        .iter()
        .filter(|n| is_test_path(&n.path))
        .map(|n| n.path.as_str())
        .collect();

    // Also test source IDs (fn:/cls: nodes whose path is a test file)
    let test_node_ids: HashSet<&str> = nodes
        .iter()
        .filter(|n| is_test_path(&n.path))
        .map(|n| n.id.as_str())
        .collect();
    let _ = test_paths; // used via test_node_ids

    // Process all edges
    for edge in edges {
        match edge.kind {
            EdgeKind::Imports => {
                // src = file:<current_file>, dst = file:<imported_file>
                // Check if dst is a valid file ID, or try to resolve it
                let dst_is_valid =
                    node_ids.contains(edge.dst.as_str()) || known_file_ids.contains(&edge.dst);

                if dst_is_valid {
                    resolved_edges.push(EdgeDef {
                        src: edge.src.clone(),
                        dst: edge.dst.clone(),
                        kind: EdgeKind::Imports,
                        ..Default::default()
                    });
                } else {
                    // Try with different extensions
                    let import_target = edge.dst.trim_start_matches("file:");
                    let mut found = false;
                    for ext in &[".ts", ".tsx", ".js", ".jsx", ".py", ".rs"] {
                        let alt = format!("file:{}{}", import_target, ext);
                        if known_file_ids.contains(&alt) {
                            resolved_edges.push(EdgeDef {
                                src: edge.src.clone(),
                                dst: alt,
                                kind: EdgeKind::Imports,
                                ..Default::default()
                            });
                            found = true;
                            break;
                        }
                    }
                    // Try directory index files (Node.js resolution)
                    if !found {
                        for index in &["/index.js", "/index.ts", "/index.jsx", "/index.tsx"] {
                            let alt = format!("file:{}{}", import_target, index);
                            if known_file_ids.contains(&alt) {
                                resolved_edges.push(EdgeDef {
                                    src: edge.src.clone(),
                                    dst: alt,
                                    kind: EdgeKind::Imports,
                                    ..Default::default()
                                });
                                found = true;
                                break;
                            }
                        }
                    }
                    if !found {
                        // Include unresolvable imports too (they may still be useful)
                        resolved_edges.push(edge.clone());
                    }
                }
            }
            EdgeKind::Exports => {
                // src = file:<path>, dst = fn:path:name or cls:path:name
                if node_ids.contains(edge.dst.as_str()) {
                    resolved_edges.push(edge.clone());
                } else {
                    // Keep the edge but log a warning — may be resolved in a later pass
                    tracing::debug!("Unresolved export edge: {} -> {}", edge.src, edge.dst);
                    resolved_edges.push(edge.clone());
                }
            }
            EdgeKind::Calls | EdgeKind::Inherits => {
                // If source node is from a test file and destination is not, use TESTS edge
                let src_is_test = test_node_ids.contains(edge.src.as_str());

                // If dst is a valid node ID, keep it. Otherwise try to resolve by name.
                if node_ids.contains(edge.dst.as_str()) {
                    let dst_is_test = test_node_ids.contains(edge.dst.as_str());
                    let kind = if src_is_test && !dst_is_test {
                        EdgeKind::Tests
                    } else {
                        edge.kind.clone()
                    };
                    resolved_edges.push(EdgeDef {
                        kind,
                        ..edge.clone()
                    });
                } else if let Some(targets) = export_index.get(&edge.dst) {
                    // Found matching names - create CALLS (or TESTS) edges with lower confidence
                    for target_id in targets {
                        let dst_is_test = test_node_ids.contains(target_id.as_str());
                        let kind = if src_is_test && !dst_is_test {
                            EdgeKind::Tests
                        } else {
                            EdgeKind::Calls
                        };
                        resolved_edges.push(EdgeDef {
                            src: edge.src.clone(),
                            dst: target_id.clone(),
                            kind,
                            confidence: 0.8,
                            ..Default::default()
                        });
                    }
                } else {
                    // Keep the edge even if unresolved (maybe a future phase can handle)
                    resolved_edges.push(edge.clone());
                }
            }
            _ => {
                // CoChanges, Owns, DependsOn — pass through unchanged
                resolved_edges.push(edge.clone());
            }
        }
    }

    Ok(resolved_edges)
}

/// Create `File` [`NodeDef`]s for every path in `file_paths`.
///
/// These synthetic nodes are added to the graph so that `IMPORTS` edges
/// always have a valid destination, even for files that contain no parseable symbols.
pub fn create_file_nodes(
    file_paths: &HashSet<String>,
    language: &HashMap<String, &str>,
) -> Vec<NodeDef> {
    let mut nodes = Vec::new();

    for path in file_paths {
        let id = format!("file:{}", path);
        let _lang = language.get(path.as_str()).copied().unwrap_or("unknown");

        nodes.push(NodeDef {
            id,
            kind: NodeKind::File,
            name: path.clone(),
            path: path.clone(),
            line_start: 1,
            line_end: 1,
            ..Default::default()
        });
    }

    nodes
}

/// Build a `file_path → language` lookup from a slice of parsed nodes.
///
/// Function and class nodes take priority over file nodes so the inferred
/// language is as accurate as possible.
pub fn build_language_map(nodes: &[NodeDef]) -> HashMap<String, &'static str> {
    let mut map = HashMap::new();
    for node in nodes {
        let lang = match node.id.split(':').next().unwrap_or("") {
            "fn" if node.path.ends_with(".ts") || node.path.ends_with(".tsx") => "typescript",
            "fn" if node.path.ends_with(".js") || node.path.ends_with(".jsx") => "javascript",
            "fn" if node.path.ends_with(".py") => "python",
            "fn" if node.path.ends_with(".rs") => "rust",
            "cls" if node.path.ends_with(".ts") || node.path.ends_with(".tsx") => "typescript",
            "cls" if node.path.ends_with(".js") || node.path.ends_with(".jsx") => "javascript",
            "cls" if node.path.ends_with(".py") => "python",
            "cls" if node.path.ends_with(".rs") => "rust",
            "file" if node.path.ends_with(".ts") || node.path.ends_with(".tsx") => "typescript",
            "file" if node.path.ends_with(".js") || node.path.ends_with(".jsx") => "javascript",
            "file" if node.path.ends_with(".py") => "python",
            "file" if node.path.ends_with(".rs") => "rust",
            _ => "unknown",
        };
        // Only insert if not already present (function/class nodes take priority)
        map.entry(node.path.clone()).or_insert(lang);
    }
    map
}
