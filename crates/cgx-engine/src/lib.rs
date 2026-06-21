pub mod bisect;
pub mod cluster;
pub mod config;
pub mod deadcode;
pub mod deps;
pub mod diff;
pub mod docs;
pub mod dupes;
pub mod export;
pub mod git;
pub mod graph;
pub mod parser;
pub mod parsers;
pub mod registry;
pub mod resolver;
pub mod rules;
pub mod skill;
pub mod timeline;
pub mod walker;

pub use cluster::{detect_communities, run_clustering};
pub use config::{
    AnalyzeConfig, CgxConfig, ChatConfig, DocsConfig, ExportConfig, IndexConfig, McpConfig,
    ProjectConfig, ServeConfig, SkillConfig, WatchConfig,
};
pub use deadcode::{
    detect_dead_code, mark_dead_candidates, Confidence, DeadCodeReport, DeadNode, DeadReason,
};
pub use deps::{audit_dependencies, parse_manifests, DependencyReport};
pub use diff::{
    compute_impact, diff_graphs, snapshot_at_commit, GraphDiff, GraphSnapshot, ImpactReport,
};
pub use dupes::{detect_clones, CloneKind, ClonePair};
pub use export::{export_dot, export_graphml, export_json, export_mermaid, export_svg};
pub use git::{analyze_repo, GitAnalysis};
pub use graph::{
    ApiScope, CloneRow, CommunityRow, CrossClusterEdge, DocsCoverage, Edge, EntryPoint,
    FileSummary, GraphDb, Node, PublicSymbol, RepoStats, SnapshotEntry, TagRow,
    TestCoverageSummary,
};
pub use parser::{
    CommentKind, CommentTag, EdgeDef, EdgeKind, LanguageParser, NodeDef, NodeKind, ParseResult,
    ParserRegistry,
};
pub use registry::{Registry, RepoEntry};
pub use resolver::{is_test_path, resolve};
pub use rules::{run_rules, Rule, RuleResult, RuleViolation, RulesConfig};
pub use skill::{
    build_skill_data, generate_agents_md, generate_skill, install_git_hooks, write_agents_md,
    write_skill, CommunityInfo, SkillData,
};
pub use timeline::build_timeline;
pub use walker::{walk_repo, Language, SourceFile};

use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Incremental repository analysis — only re-parses changed files.
/// Returns true if analysis was performed, false if no changes detected.
pub fn analyze_repo_incremental(
    repo_path: &Path,
    db: &GraphDb,
    quiet: bool,
    no_git: bool,
    no_cluster: bool,
    verbose: bool,
) -> anyhow::Result<bool> {
    let _ = verbose;

    // 1. Walk all files and compute hashes
    let files = walk_repo(repo_path)?;
    let mut current_hashes: HashMap<String, String> = HashMap::new();
    for file in &files {
        let mut hasher = Sha256::new();
        hasher.update(file.content.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        current_hashes.insert(file.relative_path.clone(), hash);
    }

    // 2. Load stored hashes
    let stored_hashes = db.get_file_hashes().unwrap_or_default();

    // 3. Determine changes
    let mut changed_paths: HashSet<String> = HashSet::new();
    for (path, hash) in &current_hashes {
        if stored_hashes.get(path) != Some(hash) {
            changed_paths.insert(path.clone());
        }
    }

    let mut deleted_paths: Vec<String> = Vec::new();
    for path in stored_hashes.keys() {
        if !current_hashes.contains_key(path) {
            deleted_paths.push(path.clone());
            changed_paths.insert(path.clone());
        }
    }

    if changed_paths.is_empty() {
        if !quiet {
            println!("  No file changes detected. Index is up to date.");
        }
        return Ok(false);
    }

    if !quiet {
        println!(
            "  Incremental: {} changed/new/deleted file(s)",
            changed_paths.len()
        );
    }

    // 4. Load existing nodes and filter out changed/deleted ones
    let existing_nodes = db.get_all_nodes()?;
    let existing_edges = db.get_all_edges()?;

    let mut kept_nodes: Vec<crate::graph::Node> = existing_nodes
        .into_iter()
        .filter(|n| !changed_paths.contains(&n.path))
        .collect();

    // 5. Parse only changed/new files
    let changed_files: Vec<_> = files
        .into_iter()
        .filter(|f| changed_paths.contains(&f.relative_path))
        .collect();

    if !quiet {
        println!("  Re-parsing {} changed file(s)...", changed_files.len());
    }

    let registry = ParserRegistry::new();
    let results = registry.parse_all(&changed_files);

    let mut new_nodes: Vec<NodeDef> = Vec::new();
    let mut new_edges: Vec<EdgeDef> = Vec::new();
    let mut changed_file_paths: HashSet<String> = HashSet::new();
    let mut lang_map: HashMap<String, &str> = changed_files
        .iter()
        .map(|f| {
            let lang_str = match f.language {
                walker::Language::TypeScript => "typescript",
                walker::Language::JavaScript => "javascript",
                walker::Language::Python => "python",
                walker::Language::Rust => "rust",
                walker::Language::Go => "go",
                walker::Language::Java => "java",
                walker::Language::CSharp => "csharp",
                walker::Language::Php => "php",
                walker::Language::Unknown => "unknown",
            };
            (f.relative_path.clone(), lang_str)
        })
        .collect();

    for result in &results {
        new_nodes.extend(result.nodes.clone());
        new_edges.extend(result.edges.clone());
    }
    for file in &changed_files {
        changed_file_paths.insert(file.relative_path.clone());
    }

    // Add file nodes for changed files
    let parsed_lang_map = resolver::build_language_map(&new_nodes);
    for (path, lang) in parsed_lang_map {
        if lang != "unknown" {
            lang_map.entry(path).or_insert(lang);
        }
    }
    let file_nodes = resolver::create_file_nodes(&changed_file_paths, &lang_map);
    new_nodes.extend(file_nodes);

    // Convert new nodes to GraphDb format
    let new_graph_nodes: Vec<crate::graph::Node> = new_nodes
        .iter()
        .map(|n| {
            let lang = lang_map.get(&n.path).copied().unwrap_or("unknown");
            crate::graph::Node::from_def(n, lang)
        })
        .collect();

    // 6. Merge kept + new nodes
    let new_node_count = new_graph_nodes.len();
    kept_nodes.extend(new_graph_nodes);

    // 7. Clear and re-insert all nodes
    db.clear()?;
    db.upsert_nodes(&kept_nodes)?;

    // Update doc_comment for nodes that have it in metadata (from changed files)
    for result in &results {
        for node_def in &result.nodes {
            if let Some(doc) = node_def
                .metadata
                .get("doc_comment")
                .and_then(|v| v.as_str())
            {
                if !doc.is_empty() {
                    let _ = db.update_node_doc_comment(&node_def.id, doc);
                }
            }
        }
    }

    // Re-convert kept nodes back to NodeDef for resolution
    let all_node_defs: Vec<NodeDef> = kept_nodes
        .iter()
        .map(|n| NodeDef {
            id: n.id.clone(),
            kind: match n.kind.as_str() {
                "File" => NodeKind::File,
                "Function" => NodeKind::Function,
                "Class" => NodeKind::Class,
                "Variable" => NodeKind::Variable,
                "Type" => NodeKind::Type,
                "Module" => NodeKind::Module,
                "Author" => NodeKind::Author,
                _ => NodeKind::Variable,
            },
            name: n.name.clone(),
            path: n.path.clone(),
            line_start: n.line_start,
            line_end: n.line_end,
            metadata: serde_json::Value::Null,
        })
        .collect();

    // Convert new edges + existing edges to EdgeDef
    let kept_edge_defs: Vec<EdgeDef> = existing_edges
        .iter()
        .filter(|e| {
            // Keep edges that don't reference changed/deleted file nodes
            let src_file = all_node_defs
                .iter()
                .find(|n| n.id == e.src)
                .map(|n| n.path.clone());
            let dst_file = all_node_defs
                .iter()
                .find(|n| n.id == e.dst)
                .map(|n| n.path.clone());
            match (src_file, dst_file) {
                (Some(sp), Some(dp)) => {
                    !changed_paths.contains(&sp) && !changed_paths.contains(&dp)
                }
                _ => false,
            }
        })
        .map(|e| EdgeDef {
            src: e.src.clone(),
            dst: e.dst.clone(),
            kind: match e.kind.as_str() {
                "CALLS" => EdgeKind::Calls,
                "IMPORTS" => EdgeKind::Imports,
                "INHERITS" => EdgeKind::Inherits,
                "EXPORTS" => EdgeKind::Exports,
                "CO_CHANGES" => EdgeKind::CoChanges,
                "OWNS" => EdgeKind::Owns,
                "DEPENDS_ON" => EdgeKind::DependsOn,
                _ => EdgeKind::Calls,
            },
            weight: e.weight,
            confidence: e.confidence,
        })
        .collect();

    let mut all_edge_defs = kept_edge_defs;
    all_edge_defs.extend(new_edges);

    // 8. Resolve cross-file symbols
    let resolved_edges = resolve(&all_node_defs, &all_edge_defs, repo_path)?;
    let resolved_count = resolved_edges.len();

    // Only upsert the resolved edge set — this matches the full-analyze flow and
    // avoids feeding DuckDB the same edge id twice in close succession, which
    // hits its INSERT OR REPLACE / ART-index bulk-delete limitation.
    let resolved_graph_edges: Vec<crate::graph::Edge> = resolved_edges
        .iter()
        .map(crate::graph::Edge::from_def)
        .collect();
    db.upsert_edges(&resolved_graph_edges)?;

    // 9. Git layer
    if !no_git {
        let all_file_paths: Vec<String> = kept_nodes
            .iter()
            .filter(|n| n.kind == "File")
            .map(|n| n.path.clone())
            .collect();
        let git_analysis = analyze_repo(repo_path, &all_file_paths)?;

        let max_churn = git_analysis
            .file_churn
            .values()
            .copied()
            .fold(0.0, f64::max);
        for (path, churn) in &git_analysis.file_churn {
            let normalized = if max_churn > 0.0 {
                churn / max_churn
            } else {
                0.0
            };
            let _ = db.upsert_node_scores(&format!("file:{}", path), normalized, 0.0);
        }

        // `file_owners` is keyed by file path; each value is that file's top
        // contributors `(author_name, email, ownership_fraction)`. Invert it
        // into one Author node per distinct contributor with an OWNS edge to
        // every file they own — otherwise we'd mint one bogus "author" per file
        // (named after the path) and point OWNS edges at author names. Authors
        // are keyed by email (matching the incremental path in cgx-cli) so name
        // variations for one identity don't fan out into separate nodes.
        let mut author_nodes = Vec::new();
        let mut own_edges = Vec::new();
        let mut seen_authors: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (file_path, owners) in &git_analysis.file_owners {
            let file_node_id = format!("file:{}", file_path);
            for (name, email, fraction) in owners {
                let author_id = format!("author:{}", email);
                if seen_authors.insert(email.clone()) {
                    author_nodes.push(crate::graph::Node {
                        id: author_id.clone(),
                        kind: "Author".to_string(),
                        name: name.clone(),
                        path: String::new(),
                        line_start: 0,
                        line_end: 0,
                        language: String::new(),
                        churn: 0.0,
                        coupling: 0.0,
                        community: 0,
                        in_degree: 0,
                        out_degree: 0,
                        exported: false,
                        is_dead_candidate: false,
                        dead_reason: None,
                        complexity: 0.0,
                        is_test_file: false,
                        test_count: 0,
                        is_tested: false,
                    });
                }
                own_edges.push(crate::graph::Edge {
                    id: format!("owns:{}:{}", author_id, file_node_id),
                    src: author_id,
                    dst: file_node_id.clone(),
                    kind: "OWNS".to_string(),
                    weight: *fraction,
                    confidence: 1.0,
                });
            }
        }
        db.upsert_nodes(&author_nodes)?;
        db.upsert_edges(&own_edges)?;

        let mut cochange_edges = Vec::new();
        for (a, b, weight) in &git_analysis.co_changes {
            cochange_edges.push(crate::graph::Edge {
                id: format!("cochange:{}:{}", a, b),
                src: format!("file:{}", a),
                dst: format!("file:{}", b),
                kind: "CO_CHANGES".to_string(),
                weight: *weight,
                confidence: 1.0,
            });
        }
        db.upsert_edges(&cochange_edges)?;
    }

    // 10. Clustering
    if !no_cluster {
        let _ = run_clustering(db)?;
    }

    // 11. Update degrees and coupling
    db.update_in_out_degrees()?;
    db.compute_coupling()?;

    // 11b. Mark test files and update test coverage
    let test_file_paths: Vec<String> = kept_nodes
        .iter()
        .filter(|n| n.kind == "File" && crate::resolver::is_test_path(&n.path))
        .map(|n| n.path.clone())
        .collect();
    // Also mark function/class nodes from test paths
    let test_node_paths: Vec<String> = kept_nodes
        .iter()
        .filter(|n| crate::resolver::is_test_path(&n.path))
        .map(|n| n.path.clone())
        .collect();
    let all_test_paths: std::collections::HashSet<String> =
        test_file_paths.into_iter().chain(test_node_paths).collect();
    db.mark_test_files(&all_test_paths.into_iter().collect::<Vec<_>>())?;
    db.update_test_coverage()?;

    // 12. Update tags for changed/deleted files
    let changed_paths_vec: Vec<String> = changed_paths.iter().cloned().collect();
    db.delete_tags_for_paths(&changed_paths_vec)?;
    let new_tag_rows: Vec<crate::graph::TagRow> = results
        .iter()
        .zip(changed_files.iter())
        .flat_map(|(result, file)| {
            result
                .comment_tags
                .iter()
                .map(move |t| crate::graph::TagRow {
                    id: format!("tag:{}:{}:{}", file.relative_path, t.line, t.tag_type),
                    file_path: file.relative_path.clone(),
                    line: t.line,
                    tag_type: t.tag_type.clone(),
                    text: t.text.clone(),
                    comment_type: t.comment_kind.as_str().to_string(),
                })
        })
        .collect();
    db.upsert_tags(&new_tag_rows)?;

    // 13. Store new file hashes
    for (path, hash) in &current_hashes {
        db.set_file_hash(path, hash)?;
    }
    if !deleted_paths.is_empty() {
        db.remove_file_hashes(&deleted_paths)?;
    }

    if !quiet {
        println!("  Incremental re-index complete.");
        println!(
            "  Kept {} unchanged nodes.",
            kept_nodes.len() - new_node_count
        );
        println!("  Added {} new/changed nodes.", new_node_count);
        if !deleted_paths.is_empty() {
            println!("  Removed {} deleted files.", deleted_paths.len());
        }
        println!("  Resolved {} cross-file edges.", resolved_count);
    }

    Ok(true)
}
