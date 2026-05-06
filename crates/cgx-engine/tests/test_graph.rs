use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use cgx_engine::{Edge, GraphDb, Node};

static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

fn temp_dir() -> PathBuf {
    let count = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "cgx-test-{}-{}",
        std::process::id(),
        count
    ));
    std::fs::create_dir_all(&dir).unwrap();

    // Create a dummy file to make the dir a "repo"
    std::fs::write(dir.join("dummy.txt"), "test").unwrap();
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
        },
    ];

    let count = db.upsert_nodes(&nodes).expect("upsert failed");
    assert_eq!(count, 2, "should insert 2 nodes");

    let total = db.node_count().expect("count failed");
    assert_eq!(total, 2, "should have 2 nodes");

    let node = db.get_node("fn:src/test.ts:hello").expect("get failed");
    assert!(node.is_some(), "should find hello node");
    let n = node.unwrap();
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
    assert!(neighbors.iter().any(|n| n.id == "fn:src/b.ts:bar"), "should find bar at depth 1");

    let neighbors2 = db
        .get_neighbors("fn:src/a.ts:foo", 2)
        .expect("get neighbors failed");
    assert!(neighbors2.iter().any(|n| n.id == "fn:src/b.ts:bar"), "should find bar at depth 2");
    assert!(neighbors2.iter().any(|n| n.id == "fn:src/c.ts:baz"), "should find baz at depth 2");

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

    let caller = db.get_node("fn:src/a.ts:caller").expect("get failed").unwrap();
    let callee = db.get_node("fn:src/b.ts:callee").expect("get failed").unwrap();

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
    };

    db.upsert_nodes(&[node1.clone()]).expect("first upsert failed");
    assert_eq!(db.node_count().unwrap(), 1);

    let node2 = Node {
        churn: 0.9,
        ..node1.clone()
    };
    db.upsert_nodes(&[node2]).expect("second upsert failed");
    assert_eq!(db.node_count().unwrap(), 1);

    let retrieved = db.get_node("fn:src/x.ts:testfn").unwrap().unwrap();
    assert!((retrieved.churn - 0.9).abs() < 0.001, "should have updated churn");

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
