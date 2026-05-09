use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use cgx_engine::{detect_communities, run_clustering, Edge, GraphDb, Node};

static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

fn temp_dir() -> PathBuf {
    let count = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir =
        std::env::temp_dir().join(format!("cgx-cluster-test-{}-{}", std::process::id(), count));
    std::fs::create_dir_all(&dir).expect("failed to create test dir");
    std::fs::write(dir.join("dummy.txt"), "test").expect("failed to write dummy file");
    dir
}

fn make_node(id: &str, kind: &str, name: &str, path: &str) -> Node {
    Node {
        id: id.to_string(),
        kind: kind.to_string(),
        name: name.to_string(),
        path: path.to_string(),
        line_start: 1,
        line_end: 1,
        language: "typescript".to_string(),
        churn: 0.0,
        coupling: 0.0,
        community: 0,
        in_degree: 0,
        out_degree: 0,
        ..Default::default()
    }
}

fn make_edge(src: &str, dst: &str, kind: &str, weight: f64) -> Edge {
    Edge {
        id: format!("{}|{}|{}", src, kind, dst),
        src: src.to_string(),
        dst: dst.to_string(),
        kind: kind.to_string(),
        weight,
        confidence: 1.0,
    }
}

#[test]
fn test_detect_communities_empty() {
    let communities = detect_communities(&[], &[]);
    assert!(communities.is_empty());
}

#[test]
fn test_detect_communities_single_node() {
    let nodes = vec![make_node("fn:src/a.ts:foo", "Function", "foo", "src/a.ts")];
    let edges = vec![];
    let communities = detect_communities(&nodes, &edges);
    assert_eq!(communities.len(), 1);
    assert_eq!(
        communities
            .get("fn:src/a.ts:foo")
            .copied()
            .expect("foo community should exist"),
        1
    );
}

#[test]
fn test_detect_communities_isolated_nodes() {
    let nodes = vec![
        make_node("fn:src/a.ts:foo", "Function", "foo", "src/a.ts"),
        make_node("fn:src/b.ts:bar", "Function", "bar", "src/b.ts"),
        make_node("fn:src/c.ts:baz", "Function", "baz", "src/c.ts"),
    ];
    let edges = vec![];
    let communities = detect_communities(&nodes, &edges);
    assert_eq!(communities.len(), 3);
    // All should be in different communities
    let unique: HashSet<_> = communities.values().collect();
    assert_eq!(
        unique.len(),
        3,
        "isolated nodes should be in separate communities"
    );
}

#[test]
fn test_detect_communities_connected_pair() {
    let nodes = vec![
        make_node("fn:src/a.ts:foo", "Function", "foo", "src/a.ts"),
        make_node("fn:src/b.ts:bar", "Function", "bar", "src/b.ts"),
    ];
    let edges = vec![make_edge(
        "fn:src/a.ts:foo",
        "fn:src/b.ts:bar",
        "CALLS",
        1.0,
    )];
    let communities = detect_communities(&nodes, &edges);
    assert_eq!(communities.len(), 2);
    // Connected pair should be in the same community
    assert_eq!(
        communities.get("fn:src/a.ts:foo"),
        communities.get("fn:src/b.ts:bar"),
        "connected nodes should be in same community"
    );
}

#[test]
fn test_detect_communities_two_clusters() {
    let nodes = vec![
        // Cluster A: a.ts functions calling each other
        make_node("fn:src/a.ts:foo", "Function", "foo", "src/a.ts"),
        make_node("fn:src/a.ts:bar", "Function", "bar", "src/a.ts"),
        make_node("fn:src/a.ts:baz", "Function", "baz", "src/a.ts"),
        // Cluster B: b.ts functions calling each other
        make_node("fn:src/b.ts:alpha", "Function", "alpha", "src/b.ts"),
        make_node("fn:src/b.ts:beta", "Function", "beta", "src/b.ts"),
        make_node("fn:src/b.ts:gamma", "Function", "gamma", "src/b.ts"),
    ];
    let edges = vec![
        // Dense connections within cluster A
        make_edge("fn:src/a.ts:foo", "fn:src/a.ts:bar", "CALLS", 1.0),
        make_edge("fn:src/a.ts:bar", "fn:src/a.ts:baz", "CALLS", 1.0),
        make_edge("fn:src/a.ts:baz", "fn:src/a.ts:foo", "CALLS", 1.0),
        // Dense connections within cluster B
        make_edge("fn:src/b.ts:alpha", "fn:src/b.ts:beta", "CALLS", 1.0),
        make_edge("fn:src/b.ts:beta", "fn:src/b.ts:gamma", "CALLS", 1.0),
        make_edge("fn:src/b.ts:gamma", "fn:src/b.ts:alpha", "CALLS", 1.0),
        // One sparse cross-cluster edge
        make_edge("fn:src/a.ts:foo", "fn:src/b.ts:alpha", "CALLS", 0.1),
    ];
    let communities = detect_communities(&nodes, &edges);

    let cluster_a = communities["fn:src/a.ts:foo"];
    let cluster_b = communities["fn:src/b.ts:alpha"];

    // All A nodes should be same community
    assert_eq!(communities["fn:src/a.ts:bar"], cluster_a);
    assert_eq!(communities["fn:src/a.ts:baz"], cluster_a);

    // All B nodes should be same community
    assert_eq!(communities["fn:src/b.ts:beta"], cluster_b);
    assert_eq!(communities["fn:src/b.ts:gamma"], cluster_b);

    // A and B should be different communities (due to sparse cross-edge)
    assert_ne!(
        cluster_a, cluster_b,
        "weakly connected clusters should separate"
    );
}

#[test]
fn test_run_clustering_integrates_with_db() {
    let dir = temp_dir();
    let db = GraphDb::open(&dir).expect("failed to open db");

    let nodes = vec![
        make_node("fn:src/auth.ts:login", "Function", "login", "src/auth.ts"),
        make_node("fn:src/auth.ts:logout", "Function", "logout", "src/auth.ts"),
        make_node("fn:src/db.ts:query", "Function", "query", "src/db.ts"),
        make_node("fn:src/db.ts:connect", "Function", "connect", "src/db.ts"),
        make_node(
            "fn:src/router.ts:handle",
            "Function",
            "handle",
            "src/router.ts",
        ),
        make_node("file:src/auth.ts", "File", "src/auth.ts", "src/auth.ts"),
        make_node("file:src/db.ts", "File", "src/db.ts", "src/db.ts"),
        make_node(
            "file:src/router.ts",
            "File",
            "src/router.ts",
            "src/router.ts",
        ),
    ];

    let edges = vec![
        make_edge("fn:src/auth.ts:login", "fn:src/db.ts:query", "CALLS", 1.0),
        make_edge("fn:src/auth.ts:logout", "fn:src/db.ts:query", "CALLS", 1.0),
        make_edge(
            "fn:src/router.ts:handle",
            "fn:src/auth.ts:login",
            "CALLS",
            1.0,
        ),
        make_edge(
            "fn:src/router.ts:handle",
            "fn:src/db.ts:connect",
            "CALLS",
            1.0,
        ),
    ];

    db.upsert_nodes(&nodes).expect("upsert nodes failed");
    db.upsert_edges(&edges).expect("upsert edges failed");

    let all_nodes = db.get_all_nodes().expect("get all nodes failed");
    let all_edges = db.get_all_edges().expect("get all edges failed");

    let community_map = detect_communities(&all_nodes, &all_edges);
    assert!(!community_map.is_empty());

    db.clear_communities().expect("clear failed");
    db.update_node_communities(&community_map)
        .expect("update communities failed");

    let communities_list = db.get_communities().expect("get communities failed");
    assert!(!communities_list.is_empty());

    let all_nodes = db.get_all_nodes().expect("get all nodes failed");
    for node in &all_nodes {
        assert!(
            node.community > 0,
            "node {} should have a community assigned",
            node.id
        );
    }

    let communities_list = db.get_communities().expect("get communities failed");
    assert!(!communities_list.is_empty());

    let unique_comms: HashSet<i64> = all_nodes.iter().map(|n| n.community).collect();
    assert!(
        unique_comms.len() >= 2,
        "should have at least 2 communities (expected more than 1)"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_clustering_stability() {
    let nodes = vec![
        make_node("fn:src/svc1.ts:a", "Function", "a", "src/svc1.ts"),
        make_node("fn:src/svc1.ts:b", "Function", "b", "src/svc1.ts"),
        make_node("fn:src/svc1.ts:c", "Function", "c", "src/svc1.ts"),
        make_node("fn:src/svc2.ts:x", "Function", "x", "src/svc2.ts"),
        make_node("fn:src/svc2.ts:y", "Function", "y", "src/svc2.ts"),
    ];
    let edges = vec![
        make_edge("fn:src/svc1.ts:a", "fn:src/svc1.ts:b", "CALLS", 1.0),
        make_edge("fn:src/svc1.ts:b", "fn:src/svc1.ts:c", "CALLS", 1.0),
        make_edge("fn:src/svc2.ts:x", "fn:src/svc2.ts:y", "CALLS", 1.0),
    ];

    // Run multiple times -- same input should produce same community count
    let mut community_counts: Vec<usize> = Vec::new();
    for _ in 0..5 {
        let communities = detect_communities(&nodes, &edges);
        let unique: HashSet<_> = communities.values().collect();
        community_counts.push(unique.len());
    }

    // Check stability: all runs should produce the same number of communities
    let first = community_counts[0];
    for &count in &community_counts[1..] {
        assert_eq!(count, first, "community count should be stable across runs");
    }

    // Should be exactly 2 communities (svc1 and svc2 clearly separated)
    assert_eq!(first, 2, "should detect exactly 2 communities");
}

#[test]
fn test_community_query_by_id() {
    let dir = temp_dir();
    let db = GraphDb::open(&dir).expect("failed to open db");

    let nodes = vec![
        make_node("fn:src/x.ts:one", "Function", "one", "src/x.ts"),
        make_node("fn:src/x.ts:two", "Function", "two", "src/x.ts"),
        make_node("fn:src/y.ts:three", "Function", "three", "src/y.ts"),
    ];
    let edges = vec![make_edge(
        "fn:src/x.ts:one",
        "fn:src/x.ts:two",
        "CALLS",
        1.0,
    )];

    db.upsert_nodes(&nodes).expect("upsert nodes failed");
    db.upsert_edges(&edges).expect("upsert edges failed");
    run_clustering(&db).expect("clustering failed");

    let first_node = db
        .get_node("fn:src/x.ts:one")
        .expect("get first node failed")
        .expect("first node should exist");
    let community_id = first_node.community;

    let community_nodes = db
        .get_nodes_by_community(community_id)
        .expect("get nodes by community failed");
    assert!(!community_nodes.is_empty());
    assert!(community_nodes.iter().any(|n| n.id == "fn:src/x.ts:one"));

    let community_edges = db
        .get_edges_by_community(community_id)
        .expect("get edges by community failed");
    assert!(!community_edges.is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_detect_communities_respects_edge_weights() {
    let nodes = vec![
        make_node("fn:src/a.ts:center", "Function", "center", "src/a.ts"),
        make_node("fn:src/b.ts:strong", "Function", "strong", "src/b.ts"),
        make_node("fn:src/c.ts:weak", "Function", "weak", "src/c.ts"),
    ];
    let edges = vec![
        make_edge("fn:src/a.ts:center", "fn:src/b.ts:strong", "CALLS", 10.0),
        make_edge("fn:src/a.ts:center", "fn:src/c.ts:weak", "CALLS", 0.1),
    ];

    let communities = detect_communities(&nodes, &edges);
    // Center should be with strong (higher weight) not with weak
    assert_eq!(
        communities["fn:src/a.ts:center"], communities["fn:src/b.ts:strong"],
        "center should cluster with strong (higher edge weight)"
    );

    let _ = std::fs::remove_dir_all(std::env::temp_dir());
}
