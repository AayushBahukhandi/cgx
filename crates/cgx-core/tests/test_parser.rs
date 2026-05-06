use cgx_core::{walk_repo, ParserRegistry};
use std::path::PathBuf;

fn fixture_path(relative: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/fixtures");
    path.push(relative);
    path
}

#[test]
fn test_ts_parser_extracts_symbols() {
    let path = fixture_path("ts-sample");
    let files = walk_repo(&path).expect("walk_repo failed");
    assert!(!files.is_empty(), "should find TS files");

    let registry = ParserRegistry::new();
    let results = registry.parse_all(&files);

    let all_names: std::collections::HashSet<String> = results
        .iter()
        .flat_map(|r| r.nodes.iter().map(|n| n.name.clone()))
        .collect();

    let required = [
        "AuthService",
        "UserService",
        "validateToken",
        "hashPassword",
        "handleLogin",
        "handleGetUser",
        "handleDeleteUser",
        "query",
        "getConnection",
    ];

    for name in &required {
        assert!(
            all_names.contains(*name),
            "missing expected node: {}",
            name
        );
    }

    let total_nodes: usize = results.iter().map(|r| r.nodes.len()).sum();
    assert!(total_nodes >= 12, "expected >= 12 nodes, got {}", total_nodes);

    let _total_edges: usize = results.iter().map(|r| r.edges.len()).sum();
    let import_edges: usize = results
        .iter()
        .flat_map(|r| r.edges.iter())
        .filter(|e| e.kind == cgx_core::EdgeKind::Imports)
        .count();
    assert!(
        import_edges >= 3,
        "expected >= 3 import edges, got {}",
        import_edges
    );
}

#[test]
fn test_py_parser_extracts_symbols() {
    let path = fixture_path("py-sample");
    let files = walk_repo(&path).expect("walk_repo failed");
    assert!(!files.is_empty(), "should find Python files");

    let registry = ParserRegistry::new();
    let results = registry.parse_all(&files);

    let all_names: std::collections::HashSet<String> = results
        .iter()
        .flat_map(|r| r.nodes.iter().map(|n| n.name.clone()))
        .collect();

    let required = [
        "AuthService",
        "User",
        "Session",
        "login",
        "logout",
        "hash_password",
        "find_by_email",
        "is_valid",
        "login_endpoint",
        "get_user_endpoint",
        "create_session",
    ];

    for name in &required {
        assert!(
            all_names.contains(*name),
            "missing expected node: {}",
            name
        );
    }

    let total_nodes: usize = results.iter().map(|r| r.nodes.len()).sum();
    assert!(total_nodes >= 10, "expected >= 10 nodes, got {}", total_nodes);
}

#[test]
fn test_rust_parser_extracts_symbols() {
    let path = fixture_path("rust-sample");
    let files = walk_repo(&path).expect("walk_repo failed");
    assert!(!files.is_empty(), "should find Rust files");

    let registry = ParserRegistry::new();
    let results = registry.parse_all(&files);

    let all_names: std::collections::HashSet<String> = results
        .iter()
        .flat_map(|r| r.nodes.iter().map(|n| n.name.clone()))
        .collect();

    let required = [
        "AuthService",
        "login",
        "logout",
        "validate_token",
        "User",
        "find_user",
        "delete_session",
    ];

    for name in &required {
        assert!(
            all_names.contains(*name),
            "missing expected node: {}",
            name
        );
    }
}

#[test]
fn test_unknown_file_does_not_panic() {
    // The walker should skip .xyz files entirely
    let path = fixture_path("ts-sample");
    let files = walk_repo(&path).expect("walk_repo failed");
    let registry = ParserRegistry::new();
    // Should not panic even with files that have unknown extensions (they're skipped)
    let _ = registry.parse_all(&files);
}

#[test]
fn test_import_edges_present() {
    let path = fixture_path("ts-sample");
    let files = walk_repo(&path).expect("walk_repo failed");
    let registry = ParserRegistry::new();
    let results = registry.parse_all(&files);

    let all_edges: Vec<_> = results.iter().flat_map(|r| r.edges.iter()).collect();
    let import_edges: Vec<_> = all_edges
        .iter()
        .filter(|e| e.kind == cgx_core::EdgeKind::Imports)
        .collect();

    assert!(
        !import_edges.is_empty(),
        "should have import edges"
    );
}
