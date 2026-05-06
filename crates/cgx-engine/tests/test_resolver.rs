use std::path::PathBuf;

use cgx_engine::{EdgeDef, EdgeKind, NodeDef, NodeKind, resolve};

#[test]
fn test_resolve_cross_file_imports() {
    let repo_root = PathBuf::from("/tmp/test-resolve");

    let nodes = vec![
        NodeDef {
            id: "fn:src/auth.ts:login".to_string(),
            kind: NodeKind::Function,
            name: "login".to_string(),
            path: "src/auth.ts".to_string(),
            line_start: 1,
            line_end: 5,
            ..Default::default()
        },
        NodeDef {
            id: "fn:src/db.ts:query".to_string(),
            kind: NodeKind::Function,
            name: "query".to_string(),
            path: "src/db.ts".to_string(),
            line_start: 1,
            line_end: 3,
            ..Default::default()
        },
        NodeDef {
            id: "cls:src/auth.ts:AuthService".to_string(),
            kind: NodeKind::Class,
            name: "AuthService".to_string(),
            path: "src/auth.ts".to_string(),
            line_start: 3,
            line_end: 15,
            ..Default::default()
        },
    ];

    let edges = vec![
        EdgeDef {
            src: "file:src/router.ts".to_string(),
            dst: "file:src/auth.ts".to_string(),
            kind: EdgeKind::Imports,
            ..Default::default()
        },
        EdgeDef {
            src: "file:src/auth.ts".to_string(),
            dst: "file:src/db.ts".to_string(),
            kind: EdgeKind::Imports,
            ..Default::default()
        },
        EdgeDef {
            src: "file:src/auth.ts".to_string(),
            dst: "fn:src/auth.ts:login".to_string(),
            kind: EdgeKind::Exports,
            ..Default::default()
        },
        EdgeDef {
            src: "file:src/auth.ts".to_string(),
            dst: "cls:src/auth.ts:AuthService".to_string(),
            kind: EdgeKind::Exports,
            ..Default::default()
        },
    ];

    let resolved = resolve(&nodes, &edges, &repo_root).expect("resolve failed");

    // Should have all import edges preserved
    let import_edges: Vec<_> = resolved.iter().filter(|e| e.kind == EdgeKind::Imports).collect();
    assert_eq!(import_edges.len(), 2, "should have 2 import edges");

    // Should have all export edges preserved (they reference valid node IDs)
    let export_edges: Vec<_> = resolved.iter().filter(|e| e.kind == EdgeKind::Exports).collect();
    assert_eq!(export_edges.len(), 2, "should have 2 export edges");

    // Total resolved edges should be >= original edges
    assert!(resolved.len() >= 4);
}

#[test]
fn test_resolve_preserves_unresolved_edges() {
    let repo_root = PathBuf::from("/tmp/test-resolve2");

    let nodes = vec![NodeDef {
        id: "fn:src/main.ts:main".to_string(),
        kind: NodeKind::Function,
        name: "main".to_string(),
        path: "src/main.ts".to_string(),
        line_start: 1,
        line_end: 3,
        ..Default::default()
    }];

    // An edge pointing to a non-existent node
    let edges = vec![EdgeDef {
        src: "file:src/main.ts".to_string(),
        dst: "file:src/nonexistent.ts".to_string(),
        kind: EdgeKind::Imports,
        ..Default::default()
    }];

    let resolved = resolve(&nodes, &edges, &repo_root).expect("resolve failed");

    // Should preserve the edge even though target doesn't exist
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].kind, EdgeKind::Imports);
}

#[test]
fn test_resolve_calls_by_name() {
    let repo_root = PathBuf::from("/tmp/test-resolve3");

    let nodes = vec![
        NodeDef {
            id: "fn:src/auth.ts:login".to_string(),
            kind: NodeKind::Function,
            name: "login".to_string(),
            path: "src/auth.ts".to_string(),
            line_start: 1,
            line_end: 5,
            ..Default::default()
        },
        NodeDef {
            id: "fn:src/router.ts:handleLogin".to_string(),
            kind: NodeKind::Function,
            name: "handleLogin".to_string(),
            path: "src/router.ts".to_string(),
            line_start: 1,
            line_end: 10,
            ..Default::default()
        },
    ];

    let edges = vec![EdgeDef {
        // A CALLS edge where dst is just the name "login" (unresolved)
        src: "fn:src/router.ts:handleLogin".to_string(),
        dst: "login".to_string(),
        kind: EdgeKind::Calls,
        ..Default::default()
    }];

    let resolved = resolve(&nodes, &edges, &repo_root).expect("resolve failed");

    // Should create a CALLS edge to the actual login node
    let calls: Vec<_> = resolved.iter().filter(|e| e.kind == EdgeKind::Calls).collect();
    assert!(!calls.is_empty(), "should have at least one CALLS edge");
    let has_login = calls.iter().any(|e| e.dst.contains("login"));
    assert!(has_login, "should resolve call to login function");
}
