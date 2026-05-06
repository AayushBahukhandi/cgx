use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::parser::{EdgeDef, EdgeKind, NodeDef, NodeKind};

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
    let known_file_ids: HashSet<String> = file_paths
        .iter()
        .map(|p| format!("file:{}", p))
        .collect();

    // Process all edges
    for edge in edges {
        match edge.kind {
            EdgeKind::Imports => {
                // src = file:<current_file>, dst = file:<imported_file>
                // Check if dst is a valid file ID, or try to resolve it
                let dst_is_valid = node_ids.contains(edge.dst.as_str())
                    || known_file_ids.contains(&edge.dst);

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
                // If dst is a valid node ID, keep it. Otherwise try to resolve by name.
                if node_ids.contains(edge.dst.as_str()) {
                    resolved_edges.push(edge.clone());
                } else if let Some(targets) = export_index.get(&edge.dst) {
                    // Found matching names - create CALLS edges with lower confidence
                    for target_id in targets {
                        resolved_edges.push(EdgeDef {
                            src: edge.src.clone(),
                            dst: target_id.clone(),
                            kind: EdgeKind::Calls,
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

pub fn create_file_nodes(
    file_paths: &HashSet<String>,
    language: &HashMap<String, &str>,
) -> Vec<NodeDef> {
    let mut nodes = Vec::new();

    for path in file_paths {
        let id = format!("file:{}", path);
        let _lang = language
            .get(path.as_str())
            .copied()
            .unwrap_or("unknown");

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

pub fn build_language_map(
    nodes: &[NodeDef],
) -> HashMap<String, &'static str> {
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
