use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use cgx_engine::{export_dot, export_graphml, export_json, export_mermaid, export_svg, Edge, GraphDb, Node};

static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

fn temp_dir() -> PathBuf {
    let count = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "cgx-export-test-{}-{}",
        std::process::id(),
        count
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("dummy.txt"), "test").unwrap();
    dir
}

fn seed_graph(db: &GraphDb) {
    let nodes = vec![
        Node {
            id: "fn:src/auth.ts:login".to_string(),
            kind: "Function".to_string(),
            name: "login".to_string(),
            path: "src/auth.ts".to_string(),
            line_start: 1,
            line_end: 5,
            language: "typescript".to_string(),
            churn: 0.8,
            coupling: 0.5,
            community: 1,
            in_degree: 2,
            out_degree: 1,
        },
        Node {
            id: "cls:src/auth.ts:AuthService".to_string(),
            kind: "Class".to_string(),
            name: "AuthService".to_string(),
            path: "src/auth.ts".to_string(),
            line_start: 3,
            line_end: 20,
            language: "typescript".to_string(),
            churn: 0.3,
            coupling: 0.7,
            community: 1,
            in_degree: 1,
            out_degree: 0,
        },
        Node {
            id: "fn:src/db.ts:query".to_string(),
            kind: "Function".to_string(),
            name: "query".to_string(),
            path: "src/db.ts".to_string(),
            line_start: 1,
            line_end: 3,
            language: "typescript".to_string(),
            churn: 0.0,
            coupling: 0.2,
            community: 2,
            in_degree: 0,
            out_degree: 0,
        },
        Node {
            id: "file:src/auth.ts".to_string(),
            kind: "File".to_string(),
            name: "src/auth.ts".to_string(),
            path: "src/auth.ts".to_string(),
            line_start: 1,
            line_end: 1,
            language: "typescript".to_string(),
            churn: 0.8,
            coupling: 0.7,
            community: 1,
            in_degree: 0,
            out_degree: 0,
        },
        Node {
            id: "file:src/db.ts".to_string(),
            kind: "File".to_string(),
            name: "src/db.ts".to_string(),
            path: "src/db.ts".to_string(),
            line_start: 1,
            line_end: 1,
            language: "typescript".to_string(),
            churn: 0.2,
            coupling: 0.0,
            community: 2,
            in_degree: 0,
            out_degree: 0,
        },
    ];

    let edges = vec![
        Edge {
            id: "fn:src/auth.ts:login|CALLS|fn:src/db.ts:query".to_string(),
            src: "fn:src/auth.ts:login".to_string(),
            dst: "fn:src/db.ts:query".to_string(),
            kind: "CALLS".to_string(),
            weight: 1.0,
            confidence: 1.0,
        },
        Edge {
            id: "fn:src/auth.ts:login|CALLS|cls:src/auth.ts:AuthService".to_string(),
            src: "fn:src/auth.ts:login".to_string(),
            dst: "cls:src/auth.ts:AuthService".to_string(),
            kind: "CALLS".to_string(),
            weight: 0.5,
            confidence: 0.9,
        },
        Edge {
            id: "file:src/auth.ts|IMPORTS|file:src/db.ts".to_string(),
            src: "file:src/auth.ts".to_string(),
            dst: "file:src/db.ts".to_string(),
            kind: "IMPORTS".to_string(),
            weight: 1.0,
            confidence: 1.0,
        },
        Edge {
            id: "file:src/auth.ts|CO_CHANGES|file:src/db.ts".to_string(),
            src: "file:src/auth.ts".to_string(),
            dst: "file:src/db.ts".to_string(),
            kind: "CO_CHANGES".to_string(),
            weight: 0.6,
            confidence: 1.0,
        },
    ];

    db.upsert_nodes(&nodes).expect("upsert nodes failed");
    db.upsert_edges(&edges).expect("upsert edges failed");
}

#[test]
fn test_export_json_valid_structure() {
    let dir = temp_dir();
    let db = GraphDb::open(&dir).expect("failed to open db");
    seed_graph(&db);

    let json = export_json(&db).expect("json export failed");
    let data: serde_json::Value = serde_json::from_str(&json).expect("invalid JSON");

    assert!(data.get("meta").is_some(), "missing meta");
    assert!(data.get("nodes").is_some(), "missing nodes");
    assert!(data.get("edges").is_some(), "missing edges");
    assert!(data.get("communities").is_some(), "missing communities");

    let meta = &data["meta"];
    assert!(meta.get("repo_id").is_some());
    assert!(meta.get("indexed_at").is_some());
    assert!(
        meta.get("node_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            >= 5
    );
    assert!(
        meta.get("edge_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            >= 4
    );

    let nodes = data["nodes"].as_array().unwrap();
    assert!(nodes.len() >= 5);
    let first_node = &nodes[0];
    assert!(first_node.get("id").is_some());
    assert!(first_node.get("kind").is_some());
    assert!(first_node.get("name").is_some());
    assert!(first_node.get("path").is_some());
    assert!(first_node.get("churn").is_some());
    assert!(first_node.get("coupling").is_some());
    assert!(first_node.get("community").is_some());

    let communities = data["communities"].as_array().unwrap();
    assert!(!communities.is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_export_json_round_trip_edges() {
    let dir = temp_dir();
    let db = GraphDb::open(&dir).expect("failed to open db");
    seed_graph(&db);

    let json = export_json(&db).expect("json export failed");
    let data: serde_json::Value = serde_json::from_str(&json).unwrap();

    let node_ids: std::collections::HashSet<&str> = data["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|n| n["id"].as_str())
        .collect();

    let edges = data["edges"].as_array().unwrap();
    for edge in edges {
        let src = edge["src"].as_str().unwrap();
        let dst = edge["dst"].as_str().unwrap();
        assert!(
            node_ids.contains(src),
            "edge source '{}' not found in nodes", src
        );
        assert!(
            node_ids.contains(dst),
            "edge destination '{}' not found in nodes", dst
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_export_mermaid_valid_syntax() {
    let dir = temp_dir();
    let db = GraphDb::open(&dir).expect("failed to open db");
    seed_graph(&db);

    let mermaid = export_mermaid(&db, 100).expect("mermaid export failed");
    assert!(
        mermaid.starts_with("graph TD"),
        "mermaid should start with 'graph TD', got: {}",
        &mermaid[..30]
    );
    assert!(mermaid.contains("-->"), "mermaid should contain arrows");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_export_mermaid_respects_max_nodes() {
    let dir = temp_dir();
    let db = GraphDb::open(&dir).expect("failed to open db");
    seed_graph(&db);

    let mermaid = export_mermaid(&db, 2).expect("mermaid export failed");
    let node_lines: Vec<&str> = mermaid
        .lines()
        .filter(|l| l.contains("[\""))
        .collect();
    assert!(node_lines.len() <= 2, "should cap at 2 nodes, got {}", node_lines.len());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_export_mermaid_empty_graph() {
    let dir = temp_dir();
    let db = GraphDb::open(&dir).expect("failed to open db");

    let mermaid = export_mermaid(&db, 100).expect("mermaid export failed");
    assert!(mermaid.contains("No data"), "empty graph should show 'No data'");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_export_dot_valid_syntax() {
    let dir = temp_dir();
    let db = GraphDb::open(&dir).expect("failed to open db");
    seed_graph(&db);

    let dot = export_dot(&db).expect("dot export failed");
    assert!(dot.starts_with("digraph"), "dot should start with 'digraph'");
    assert!(dot.contains("->"), "dot should contain arrows");
    assert!(dot.ends_with("}\n"), "dot should end with closing brace");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_export_dot_empty_graph() {
    let dir = temp_dir();
    let db = GraphDb::open(&dir).expect("failed to open db");

    let dot = export_dot(&db).expect("dot export failed");
    assert!(dot.contains("No data"), "empty graph should indicate no data");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_export_svg_valid_xml() {
    let dir = temp_dir();
    let db = GraphDb::open(&dir).expect("failed to open db");
    seed_graph(&db);

    let svg = export_svg(&db).expect("svg export failed");
    assert!(svg.starts_with("<svg"), "svg should start with '<svg'");
    assert!(svg.contains("</svg>"), "svg should close with '</svg>'");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_export_svg_empty_graph() {
    let dir = temp_dir();
    let db = GraphDb::open(&dir).expect("failed to open db");

    let svg = export_svg(&db).expect("svg export failed");
    assert!(svg.contains("No data") || svg.contains("0 nodes"), "should show empty state");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_export_graphml_valid_structure() {
    let dir = temp_dir();
    let db = GraphDb::open(&dir).expect("failed to open db");
    seed_graph(&db);

    let graphml = export_graphml(&db).expect("graphml export failed");
    assert!(
        graphml.starts_with("<?xml"),
        "GraphML should start with XML declaration"
    );
    assert!(graphml.contains("<graphml"), "should contain graphml element");
    assert!(graphml.contains("<node"), "should contain nodes");
    assert!(graphml.contains("<edge"), "should contain edges");
    assert!(graphml.contains("</graphml>"), "should close graphml");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_all_exports_succeed_on_empty_db() {
    let dir = temp_dir();
    let db = GraphDb::open(&dir).expect("failed to open db");

    let json = export_json(&db).expect("json failed");
    assert!(json.contains("\"nodes\""));

    let mermaid = export_mermaid(&db, 100).expect("mermaid failed");
    assert!(!mermaid.is_empty());

    let dot = export_dot(&db).expect("dot failed");
    assert!(!dot.is_empty());

    let svg = export_svg(&db).expect("svg failed");
    assert!(!svg.is_empty());

    let graphml = export_graphml(&db).expect("graphml failed");
    assert!(!graphml.is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_export_json_includes_all_kinds() {
    let dir = temp_dir();
    let db = GraphDb::open(&dir).expect("failed to open db");
    seed_graph(&db);

    let json = export_json(&db).expect("json export failed");
    let data: serde_json::Value = serde_json::from_str(&json).unwrap();

    let kinds: std::collections::HashSet<&str> = data["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|n| n["kind"].as_str())
        .collect();

    assert!(kinds.contains("Function"), "should have Function nodes");
    assert!(kinds.contains("Class"), "should have Class nodes");
    assert!(kinds.contains("File"), "should have File nodes");

    let edge_kinds: std::collections::HashSet<&str> = data["edges"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["kind"].as_str())
        .collect();

    assert!(edge_kinds.contains("CALLS"));
    assert!(edge_kinds.contains("IMPORTS"));
    assert!(edge_kinds.contains("CO_CHANGES"));

    let _ = std::fs::remove_dir_all(&dir);
}
