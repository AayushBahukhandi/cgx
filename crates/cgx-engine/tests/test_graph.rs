use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use cgx_engine::{Edge, GraphDb, Node};

static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

fn temp_dir() -> PathBuf {
    let count = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("cgx-test-{}-{}", std::process::id(), count));
    std::fs::create_dir_all(&dir).expect("failed to create test dir");

    // Create a dummy file to make the dir a "repo"
    std::fs::write(dir.join("dummy.txt"), "test").expect("failed to write dummy file");
    dir
}

#[test]
fn test_insert_and_query_nodes() {
    let dir = temp_dir();
    let db = GraphDb::open(&dir).expect("failed to open db");

    let nodes = vec![
        Node {
            id: "fn:src/test.ts:hello".to_string(),
            kind: "Function".to_string(),
            name: "hello".to_string(),
            path: "src/test.ts".to_string(),
            line_start: 1,
            line_end: 5,
            language: "typescript".to_string(),
            churn: 0.5,
            coupling: 0.3,
            community: 0,
            in_degree: 0,
            out_degree: 0,
            ..Default::default()
        },
        Node {
            id: "fn:src/test.ts:world".to_string(),
            kind: "Function".to_string(),
            name: "world".to_string(),
            path: "src/test.ts".to_string(),
            line_start: 6,
            line_end: 10,
            language: "typescript".to_string(),
            churn: 0.0,
            coupling: 0.0,
            community: 0,
            in_degree: 0,
            out_degree: 0,
            ..Default::default()
        },
    ];

    let count = db.upsert_nodes(&nodes).expect("upsert failed");
    assert_eq!(count, 2, "should insert 2 nodes");

    let total = db.node_count().expect("count failed");
    assert_eq!(total, 2, "should have 2 nodes");

    let node = db.get_node("fn:src/test.ts:hello").expect("get failed");
    assert!(node.is_some(), "should find hello node");
    let n = node.expect("hello node should exist");
    assert_eq!(n.name, "hello");
    assert_eq!(n.kind, "Function");
    assert!((n.churn - 0.5).abs() < 0.001);

    // Cleanup
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_insert_and_query_edges() {
    let dir = temp_dir();
    let db = GraphDb::open(&dir).expect("failed to open db");

    let nodes = vec![
        Node {
            id: "fn:src/a.ts:foo".to_string(),
            kind: "Function".to_string(),
            name: "foo".to_string(),
            path: "src/a.ts".to_string(),
            line_start: 1,
            line_end: 3,
            language: "typescript".to_string(),
            churn: 0.0,
            coupling: 0.0,
            community: 0,
            in_degree: 0,
            out_degree: 0,
            ..Default::default()
        },
        Node {
            id: "fn:src/b.ts:bar".to_string(),
            kind: "Function".to_string(),
            name: "bar".to_string(),
            path: "src/b.ts".to_string(),
            line_start: 1,
            line_end: 3,
            language: "typescript".to_string(),
            churn: 0.0,
            coupling: 0.0,
            community: 0,
            in_degree: 0,
            out_degree: 0,
            ..Default::default()
        },
    ];

    db.upsert_nodes(&nodes).expect("upsert nodes failed");

    let edges = vec![Edge {
        id: "fn:src/a.ts:foo|CALLS|fn:src/b.ts:bar".to_string(),
        src: "fn:src/a.ts:foo".to_string(),
        dst: "fn:src/b.ts:bar".to_string(),
        kind: "CALLS".to_string(),
        weight: 1.0,
        confidence: 0.9,
    }];

    let count = db.upsert_edges(&edges).expect("upsert edges failed");
    assert_eq!(count, 1, "should insert 1 edge");

    let total = db.edge_count().expect("count failed");
    assert_eq!(total, 1, "should have 1 edge");

    let all_edges = db.get_all_edges().expect("get all edges failed");
    assert_eq!(all_edges.len(), 1);
    assert_eq!(all_edges[0].kind, "CALLS");
    assert!((all_edges[0].confidence - 0.9).abs() < 0.001);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_get_neighbors() {
    let dir = temp_dir();
    let db = GraphDb::open(&dir).expect("failed to open db");

    let nodes = vec![
        Node {
            id: "fn:src/a.ts:foo".to_string(),
            kind: "Function".to_string(),
            name: "foo".to_string(),
            path: "src/a.ts".to_string(),
            line_start: 1,
            line_end: 3,
            language: "typescript".to_string(),
            churn: 0.0,
            coupling: 0.0,
            community: 0,
            in_degree: 0,
            out_degree: 0,
            ..Default::default()
        },
        Node {
            id: "fn:src/b.ts:bar".to_string(),
            kind: "Function".to_string(),
            name: "bar".to_string(),
            path: "src/b.ts".to_string(),
            line_start: 1,
            line_end: 3,
            language: "typescript".to_string(),
            churn: 0.0,
            coupling: 0.0,
            community: 0,
            in_degree: 0,
            out_degree: 0,
            ..Default::default()
        },
        Node {
            id: "fn:src/c.ts:baz".to_string(),
            kind: "Function".to_string(),
            name: "baz".to_string(),
            path: "src/c.ts".to_string(),
            line_start: 1,
            line_end: 3,
            language: "typescript".to_string(),
            churn: 0.0,
            coupling: 0.0,
            community: 0,
            in_degree: 0,
            out_degree: 0,
            ..Default::default()
        },
    ];

    db.upsert_nodes(&nodes).expect("upsert nodes failed");

    let edges = vec![
        Edge {
            id: "e1".to_string(),
            src: "fn:src/a.ts:foo".to_string(),
            dst: "fn:src/b.ts:bar".to_string(),
            kind: "CALLS".to_string(),
            weight: 1.0,
            confidence: 1.0,
        },
        Edge {
            id: "e2".to_string(),
            src: "fn:src/b.ts:bar".to_string(),
            dst: "fn:src/c.ts:baz".to_string(),
            kind: "CALLS".to_string(),
            weight: 1.0,
            confidence: 1.0,
        },
    ];

    db.upsert_edges(&edges).expect("upsert edges failed");

    let neighbors = db
        .get_neighbors("fn:src/a.ts:foo", 1)
        .expect("get neighbors failed");
    assert!(
        neighbors.iter().any(|n| n.id == "fn:src/b.ts:bar"),
        "should find bar at depth 1"
    );

    let neighbors2 = db
        .get_neighbors("fn:src/a.ts:foo", 2)
        .expect("get neighbors failed");
    assert!(
        neighbors2.iter().any(|n| n.id == "fn:src/b.ts:bar"),
        "should find bar at depth 2"
    );
    assert!(
        neighbors2.iter().any(|n| n.id == "fn:src/c.ts:baz"),
        "should find baz at depth 2"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_node_count_empty() {
    let dir = temp_dir();
    let db = GraphDb::open(&dir).expect("failed to open db");
    db.clear().expect("failed to clear");

    let count = db.node_count().expect("count failed");
    assert_eq!(count, 0, "should be 0 after clear");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_update_in_out_degrees() {
    let dir = temp_dir();
    let db = GraphDb::open(&dir).expect("failed to open db");

    let nodes = vec![
        Node {
            id: "fn:src/a.ts:caller".to_string(),
            kind: "Function".to_string(),
            name: "caller".to_string(),
            path: "src/a.ts".to_string(),
            line_start: 1,
            line_end: 3,
            language: "typescript".to_string(),
            churn: 0.0,
            coupling: 0.0,
            community: 0,
            in_degree: 0,
            out_degree: 0,
            ..Default::default()
        },
        Node {
            id: "fn:src/b.ts:callee".to_string(),
            kind: "Function".to_string(),
            name: "callee".to_string(),
            path: "src/b.ts".to_string(),
            line_start: 1,
            line_end: 3,
            language: "typescript".to_string(),
            churn: 0.0,
            coupling: 0.0,
            community: 0,
            in_degree: 0,
            out_degree: 0,
            ..Default::default()
        },
    ];

    db.upsert_nodes(&nodes).expect("upsert nodes failed");

    let edges = vec![Edge {
        id: "call_edge".to_string(),
        src: "fn:src/a.ts:caller".to_string(),
        dst: "fn:src/b.ts:callee".to_string(),
        kind: "CALLS".to_string(),
        weight: 1.0,
        confidence: 1.0,
    }];

    db.upsert_edges(&edges).expect("upsert edges failed");
    db.update_in_out_degrees().expect("update failed");

    let caller = db
        .get_node("fn:src/a.ts:caller")
        .expect("get caller failed")
        .expect("caller should exist");
    let callee = db
        .get_node("fn:src/b.ts:callee")
        .expect("get callee failed")
        .expect("callee should exist");

    assert_eq!(caller.out_degree, 1, "caller should have out_degree 1");
    assert_eq!(caller.in_degree, 0, "caller should have in_degree 0");
    assert_eq!(callee.in_degree, 1, "callee should have in_degree 1");
    assert_eq!(callee.out_degree, 0, "callee should have out_degree 0");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_upsert_replaces_existing() {
    let dir = temp_dir();
    let db = GraphDb::open(&dir).expect("failed to open db");

    let node1 = Node {
        id: "fn:src/x.ts:testfn".to_string(),
        kind: "Function".to_string(),
        name: "testfn".to_string(),
        path: "src/x.ts".to_string(),
        line_start: 1,
        line_end: 2,
        language: "typescript".to_string(),
        churn: 0.1,
        coupling: 0.2,
        community: 0,
        in_degree: 0,
        out_degree: 0,
        ..Default::default()
    };

    db.upsert_nodes(std::slice::from_ref(&node1))
        .expect("first upsert failed");
    assert_eq!(db.node_count().expect("node count failed"), 1);

    let node2 = Node {
        churn: 0.9,
        ..node1.clone()
    };
    db.upsert_nodes(&[node2]).expect("second upsert failed");
    assert_eq!(db.node_count().expect("node count failed"), 1);

    let retrieved = db
        .get_node("fn:src/x.ts:testfn")
        .expect("get updated node failed")
        .expect("updated node should exist");
    assert!(
        (retrieved.churn - 0.9).abs() < 0.001,
        "should have updated churn"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_get_language_breakdown() {
    let dir = temp_dir();
    let db = GraphDb::open(&dir).expect("failed to open db");

    let nodes = vec![
        Node {
            id: "fn:src/a.ts:foo".to_string(),
            kind: "Function".to_string(),
            name: "foo".to_string(),
            path: "src/a.ts".to_string(),
            line_start: 1,
            line_end: 3,
            language: "typescript".to_string(),
            churn: 0.0,
            coupling: 0.0,
            community: 0,
            in_degree: 0,
            out_degree: 0,
            ..Default::default()
        },
        Node {
            id: "fn:src/b.py:bar".to_string(),
            kind: "Function".to_string(),
            name: "bar".to_string(),
            path: "src/b.py".to_string(),
            line_start: 1,
            line_end: 3,
            language: "python".to_string(),
            churn: 0.0,
            coupling: 0.0,
            community: 0,
            in_degree: 0,
            out_degree: 0,
            ..Default::default()
        },
        Node {
            id: "fn:src/c.py:baz".to_string(),
            kind: "Function".to_string(),
            name: "baz".to_string(),
            path: "src/c.py".to_string(),
            line_start: 1,
            line_end: 3,
            language: "python".to_string(),
            churn: 0.0,
            coupling: 0.0,
            community: 0,
            in_degree: 0,
            out_degree: 0,
            ..Default::default()
        },
    ];

    db.upsert_nodes(&nodes).expect("upsert nodes failed");

    let breakdown = db.get_language_breakdown().expect("breakdown failed");
    assert!(!breakdown.is_empty());
    let ts_share = breakdown.get("typescript").copied().unwrap_or(0.0);
    let py_share = breakdown.get("python").copied().unwrap_or(0.0);
    assert!((ts_share - 1.0 / 3.0).abs() < 0.01);
    assert!((py_share - 2.0 / 3.0).abs() < 0.01);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_open_migration_adds_dead_columns() {
    // Simulate an old DB created without exported/is_dead_candidate/dead_reason columns.
    // The migration in GraphDb::open() must add them idempotently.
    let dir = temp_dir();
    let db_path = dir.join("graph.db");

    // Create OLD schema (without new columns).
    {
        let conn = duckdb::Connection::open(&db_path).expect("open raw conn");
        conn.execute_batch(
            "CREATE TABLE nodes (
                id VARCHAR PRIMARY KEY,
                kind VARCHAR NOT NULL,
                name VARCHAR NOT NULL,
                path VARCHAR NOT NULL,
                line_start INTEGER,
                line_end INTEGER,
                language VARCHAR,
                churn DOUBLE DEFAULT 0.0,
                coupling DOUBLE DEFAULT 0.0,
                community BIGINT DEFAULT 0,
                in_degree BIGINT DEFAULT 0,
                out_degree BIGINT DEFAULT 0
            );
            CREATE TABLE edges (
                id VARCHAR PRIMARY KEY,
                src VARCHAR NOT NULL,
                dst VARCHAR NOT NULL,
                kind VARCHAR NOT NULL,
                weight DOUBLE DEFAULT 1.0,
                confidence DOUBLE DEFAULT 1.0
            );",
        )
        .expect("create old schema");
    }

    // Reopen and run the migration (same SQL as GraphDb::open).
    {
        let conn = duckdb::Connection::open(&db_path).expect("reopen");
        conn.execute_batch(
            "ALTER TABLE nodes ADD COLUMN IF NOT EXISTS exported           BOOLEAN DEFAULT false;
             ALTER TABLE nodes ADD COLUMN IF NOT EXISTS is_dead_candidate  BOOLEAN DEFAULT false;
             ALTER TABLE nodes ADD COLUMN IF NOT EXISTS dead_reason        TEXT;
             CREATE INDEX IF NOT EXISTS idx_nodes_dead ON nodes(is_dead_candidate);",
        )
        .expect("migration must succeed on old schema");

        // Verify column exists
        conn.prepare("SELECT COALESCE(is_dead_candidate, false) FROM nodes LIMIT 0")
            .expect("is_dead_candidate must be queryable after migration");

        // Verify idempotent: running migration again must also succeed
        conn.execute_batch(
            "ALTER TABLE nodes ADD COLUMN IF NOT EXISTS exported           BOOLEAN DEFAULT false;
             ALTER TABLE nodes ADD COLUMN IF NOT EXISTS is_dead_candidate  BOOLEAN DEFAULT false;
             ALTER TABLE nodes ADD COLUMN IF NOT EXISTS dead_reason        TEXT;
             CREATE INDEX IF NOT EXISTS idx_nodes_dead ON nodes(is_dead_candidate);",
        )
        .expect("second migration run must be idempotent");
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_exported_persisted_in_db() {
    let tmp = temp_dir();
    let db = GraphDb::open(&tmp).unwrap();

    // Create a node with exported=true
    let node_with_export = Node {
        id: "fn:test.ts:myFunc".to_string(),
        kind: "Function".to_string(),
        name: "myFunc".to_string(),
        path: "test.ts".to_string(),
        line_start: 1,
        line_end: 5,
        language: "typescript".to_string(),
        churn: 0.0,
        coupling: 0.0,
        community: 0,
        in_degree: 0,
        out_degree: 0,
        exported: true,
        is_dead_candidate: false,
        dead_reason: None,
        complexity: 0.0,
        is_test_file: false,
        test_count: 0,
        is_tested: false,
    };

    db.upsert_nodes(&[node_with_export]).unwrap();

    let nodes = db.get_all_nodes().unwrap();
    let n = nodes.iter().find(|n| n.name == "myFunc").unwrap();
    assert!(n.exported, "exported should be true after upsert");

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn test_full_analyze_preserves_exported() {
    use cgx_engine::graph::Node;
    use cgx_engine::resolver::{build_language_map, create_file_nodes};
    use cgx_engine::{walk_repo, GraphDb, ParserRegistry};
    use std::collections::HashSet;
    use std::path::PathBuf;

    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ts-sample");

    let files = walk_repo(&fixture).unwrap();
    let registry = ParserRegistry::new();
    let results = registry.parse_all(&files);

    let mut all_nodes = Vec::new();
    let mut all_edges = Vec::new();
    let mut file_paths = HashSet::new();

    for result in &results {
        all_nodes.extend(result.nodes.clone());
        all_edges.extend(result.edges.clone());
    }
    for file in &files {
        file_paths.insert(file.relative_path.clone());
    }

    let lang_map: std::collections::HashMap<String, &str> = files
        .iter()
        .map(|f| (f.relative_path.clone(), "typescript"))
        .collect();

    let file_nodes = create_file_nodes(&file_paths, &lang_map);
    all_nodes.extend(file_nodes);

    // Check that unusedExportedHelper has exported=true in NodeDef
    let helper_def = all_nodes.iter().find(|n| n.name == "unusedExportedHelper");
    if let Some(def) = helper_def {
        let exported = def
            .metadata
            .get("exported")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        eprintln!("NodeDef unusedExportedHelper exported={}", exported);
    }

    // Build db_nodes
    let db_nodes: Vec<Node> = all_nodes
        .iter()
        .map(|n| {
            let lang = lang_map.get(&n.path).copied().unwrap_or("unknown");
            Node::from_def(n, lang)
        })
        .collect();

    // Check db_nodes
    let helper_node = db_nodes.iter().find(|n| n.name == "unusedExportedHelper");
    assert!(helper_node.is_some(), "should have unusedExportedHelper");
    let hn = helper_node.unwrap();
    eprintln!("Node unusedExportedHelper exported={}", hn.exported);
    assert!(
        hn.exported,
        "unusedExportedHelper should have exported=true in Node"
    );

    // Now store in DB and read back
    let tmp = temp_dir();
    let db = GraphDb::open(&tmp).unwrap();
    db.clear().unwrap();
    db.upsert_nodes(&db_nodes).unwrap();

    let all_stored = db.get_all_nodes().unwrap();
    let stored_helper = all_stored.iter().find(|n| n.name == "unusedExportedHelper");
    assert!(
        stored_helper.is_some(),
        "should find unusedExportedHelper in DB"
    );
    let sh = stored_helper.unwrap();
    eprintln!("DB node unusedExportedHelper exported={}", sh.exported);
    assert!(
        sh.exported,
        "unusedExportedHelper.exported should be true in DB"
    );

    std::fs::remove_dir_all(tmp).ok();
}

#[test]
fn test_dead_code_e2e() {
    use cgx_engine::graph::Node;
    use cgx_engine::resolver::{build_language_map, create_file_nodes};
    use cgx_engine::{detect_dead_code, mark_dead_candidates, walk_repo, GraphDb, ParserRegistry};
    use std::collections::HashSet;
    use std::path::PathBuf;

    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ts-sample");

    let files = walk_repo(&fixture).unwrap();
    let registry = ParserRegistry::new();
    let results = registry.parse_all(&files);

    let mut all_nodes = Vec::new();
    let mut all_edges = Vec::new();
    let mut file_paths = HashSet::new();

    for result in &results {
        all_nodes.extend(result.nodes.clone());
        all_edges.extend(result.edges.clone());
    }
    for file in &files {
        file_paths.insert(file.relative_path.clone());
    }

    let lang_map: std::collections::HashMap<String, &str> = files
        .iter()
        .map(|f| (f.relative_path.clone(), "typescript"))
        .collect();
    let file_nodes = create_file_nodes(&file_paths, &lang_map);
    all_nodes.extend(file_nodes);

    let db_nodes: Vec<Node> = all_nodes
        .iter()
        .map(|n| {
            let lang = lang_map.get(&n.path).copied().unwrap_or("unknown");
            Node::from_def(n, lang)
        })
        .collect();

    let resolved_edges = cgx_engine::resolve(&all_nodes, &all_edges, &fixture).unwrap();
    let db_edges: Vec<cgx_engine::Edge> = resolved_edges
        .iter()
        .map(cgx_engine::Edge::from_def)
        .collect();

    let tmp = temp_dir();
    let db = GraphDb::open(&tmp).unwrap();
    db.clear().unwrap();
    db.upsert_nodes(&db_nodes).unwrap();
    db.upsert_edges(&db_edges).unwrap();
    db.update_in_out_degrees().unwrap();

    // Check stored values
    let all_stored = db.get_all_nodes().unwrap();
    for n in &all_stored {
        if n.name == "unusedExportedHelper" || n.name == "_neverCalledPrivate" {
            eprintln!(
                "{} exported={} in_degree={} out_degree={}",
                n.name, n.exported, n.in_degree, n.out_degree
            );
        }
    }

    let report = detect_dead_code(&db).unwrap();
    eprintln!(
        "unreferenced_exports: {}",
        report.unreferenced_exports.len()
    );
    eprintln!("unreachable: {}", report.unreachable.len());

    let found_private = report
        .unreachable
        .iter()
        .any(|dn| dn.node.name == "_neverCalledPrivate");
    let found_helper = report
        .unreferenced_exports
        .iter()
        .any(|dn| dn.node.name == "unusedExportedHelper");

    eprintln!(
        "found_private={} found_helper={}",
        found_private, found_helper
    );

    std::fs::remove_dir_all(tmp).ok();
    assert!(
        found_private,
        "_neverCalledPrivate should be detected as unreachable"
    );
    assert!(
        found_helper,
        "unusedExportedHelper should be detected as unreferenced export"
    );
}
