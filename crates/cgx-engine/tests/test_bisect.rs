//! Tests for the bisect predicate evaluator.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use cgx_engine::bisect::BisectPredicates;
use cgx_engine::{Edge, GraphDb, Node};

static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

fn temp_dir() -> PathBuf {
    let count = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir =
        std::env::temp_dir().join(format!("cgx-bisect-test-{}-{}", std::process::id(), count));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn make_node(id: &str, name: &str, path: &str, is_dead: bool) -> Node {
    Node {
        id: id.to_string(),
        kind: "Function".to_string(),
        name: name.to_string(),
        path: path.to_string(),
        line_start: 1,
        line_end: 5,
        language: "rust".to_string(),
        churn: 0.0,
        coupling: 0.0,
        community: 1,
        in_degree: 0,
        out_degree: 0,
        exported: true,
        is_dead_candidate: is_dead,
        dead_reason: if is_dead {
            Some("test".to_string())
        } else {
            None
        },
        complexity: 0.0,
        is_test_file: false,
        test_count: 0,
        is_tested: false,
    }
}

fn seed(db: &GraphDb) {
    db.upsert_nodes(&[
        make_node("fn:src/a.rs:alive_fn", "alive_fn", "src/a.rs", false),
        make_node("fn:src/b.rs:dead_fn", "dead_fn", "src/b.rs", true),
    ])
    .expect("upsert");
    db.upsert_edges(&[Edge {
        id: "fn:src/a.rs:alive_fn|CALLS|fn:src/b.rs:dead_fn".to_string(),
        src: "fn:src/a.rs:alive_fn".to_string(),
        dst: "fn:src/b.rs:dead_fn".to_string(),
        kind: "CALLS".to_string(),
        weight: 1.0,
        confidence: 1.0,
    }])
    .expect("upsert edges");
    // `upsert_nodes` doesn't write the `is_dead_candidate` column; it's set by
    // the deadcode pass. Inject the bit directly so the predicate evaluator
    // has something to read.
    db.conn
        .execute_batch("UPDATE nodes SET is_dead_candidate = 1 WHERE id = 'fn:src/b.rs:dead_fn'")
        .expect("flag dead");
}

#[test]
fn test_predicates_pass_when_all_satisfied() {
    let dir = temp_dir();
    let db = GraphDb::open(&dir).expect("open");
    seed(&db);

    let preds = BisectPredicates {
        node_count_min: Some(1),
        node_count_max: Some(100),
        nodes_exist: vec!["fn:src/a.rs:alive_fn".to_string()],
        nodes_missing: vec!["fn:src/c.rs:not_there".to_string()],
        nodes_alive: vec!["fn:src/a.rs:alive_fn".to_string()],
        rule_violations_max: None,
    };
    let report = preds.evaluate(&db).expect("evaluate");
    assert!(
        report.passed(),
        "all predicates should pass: {:?}",
        report.outcomes
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_dead_node_fails_alive_predicate() {
    let dir = temp_dir();
    let db = GraphDb::open(&dir).expect("open");
    seed(&db);

    let preds = BisectPredicates {
        nodes_alive: vec!["fn:src/b.rs:dead_fn".to_string()],
        ..Default::default()
    };
    let report = preds.evaluate(&db).expect("evaluate");
    assert!(!report.passed(), "dead_fn is dead, should fail");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_missing_node_fails_exist_predicate() {
    let dir = temp_dir();
    let db = GraphDb::open(&dir).expect("open");
    seed(&db);

    let preds = BisectPredicates {
        nodes_exist: vec!["fn:nowhere.rs:ghost".to_string()],
        ..Default::default()
    };
    let report = preds.evaluate(&db).expect("evaluate");
    assert!(!report.passed());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_load_from_toml() {
    let dir = temp_dir();
    std::fs::create_dir_all(dir.join(".cgx")).expect("mkdir");
    std::fs::write(
        dir.join(".cgx/bisect.toml"),
        r#"
node_count_min = 5
nodes_exist = ["fn:x:y"]
"#,
    )
    .expect("write toml");
    let preds = BisectPredicates::load(&dir, None).expect("load");
    assert_eq!(preds.node_count_min, Some(5));
    assert_eq!(preds.nodes_exist, vec!["fn:x:y".to_string()]);
    let _ = std::fs::remove_dir_all(&dir);
}
