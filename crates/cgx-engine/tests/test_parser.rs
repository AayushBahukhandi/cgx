use cgx_engine::{walk_repo, ParserRegistry};
use std::path::PathBuf;

fn fixture_path(relative: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/fixtures");
    path.push(relative);
    path
}

#[test]
fn test_ts_exported_metadata() {
    let files = walk_repo(&fixture_path("ts-sample")).unwrap();
    let registry = ParserRegistry::new();
    for file in &files {
        if file.relative_path.ends_with("user.ts") {
            let result = registry.parse(file).unwrap();
            let mut found_private = false;
            let mut found_exported_helper = false;
            for node in &result.nodes {
                let exported = node
                    .metadata
                    .get("exported")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                eprintln!(
                    "NODE {} {} exported={}",
                    node.kind.as_str(),
                    node.name,
                    exported
                );
                if node.name == "_neverCalledPrivate" {
                    found_private = true;
                    assert!(!exported, "_neverCalledPrivate should not be exported");
                }
                if node.name == "unusedExportedHelper" {
                    found_exported_helper = true;
                    assert!(exported, "unusedExportedHelper should be exported");
                }
            }
            assert!(found_private, "should have found _neverCalledPrivate");
            assert!(
                found_exported_helper,
                "should have found unusedExportedHelper"
            );
        }
    }
}

#[test]
fn debug_ts_tree_structure() {
    use cgx_engine::walk_repo;
    use std::path::PathBuf;

    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ts-sample");
    let files = walk_repo(&fixture).unwrap();

    for file in &files {
        if file.relative_path.ends_with("user.ts") {
            // Parse with tree-sitter manually to see AST
            let language = tree_sitter_typescript::language_typescript();
            let mut parser = tree_sitter::Parser::new();
            parser.set_language(&language).unwrap();
            let tree = parser.parse(&file.content, None).unwrap();
            let root = tree.root_node();
            let source = file.content.as_bytes();

            // Walk root children
            for i in 0..root.child_count() {
                if let Some(child) = root.child(i) {
                    if child.kind() == "export_statement" {
                        eprintln!(
                            "=== export_statement at line {} ===",
                            child.start_position().row + 1
                        );
                        for j in 0..child.child_count() {
                            if let Some(ec) = child.child(j) {
                                let text = &source[ec.byte_range()];
                                let text_str = std::str::from_utf8(text).unwrap_or("?");
                                let preview = &text_str[..text_str.len().min(40)];
                                eprintln!("  child[{}] kind={} text={:?}", j, ec.kind(), preview);
                                // Also check named children
                                for k in 0..ec.child_count() {
                                    if let Some(ecc) = ec.child(k) {
                                        let t = std::str::from_utf8(&source[ecc.byte_range()])
                                            .unwrap_or("?");
                                        eprintln!(
                                            "    child[{}] kind={} name={}",
                                            k,
                                            ecc.kind(),
                                            &t[..t.len().min(30)]
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
