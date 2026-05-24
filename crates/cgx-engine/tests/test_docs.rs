//! Smoke test for the docs vault generator. Seeds a minimal in-memory graph,
//! writes a vault to a temp directory, and asserts the expected layout + that
//! every module note ends with a fillable AI prose stub.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use cgx_engine::docs::{generate_vault, DocsMode, DocsOptions, DocsTarget, WikiLinkStyle};
use cgx_engine::{Edge, GraphDb, Node};

static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

fn temp_repo_dir() -> PathBuf {
    let count = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("cgx-docs-test-{}-{}", std::process::id(), count));
    std::fs::create_dir_all(&dir).expect("create temp repo dir");
    // The docs/project module reads README.md / Cargo.toml for the overview;
    // give it tiny ones so the smoke test exercises that path too.
    std::fs::write(
        dir.join("README.md"),
        "# Test Project\n\nA fixture repo for the cgx docs smoke test.\n",
    )
    .expect("write README");
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"smoke-test\"\nversion = \"0.0.1\"\ndescription = \"smoke\"\n\n[dependencies]\nserde = \"1\"\n",
    )
    .expect("write Cargo.toml");
    dir
}

fn make_node(id: &str, kind: &str, name: &str, path: &str) -> Node {
    Node {
        id: id.to_string(),
        kind: kind.to_string(),
        name: name.to_string(),
        path: path.to_string(),
        line_start: 1,
        line_end: 10,
        language: "rust".to_string(),
        churn: 0.5,
        coupling: 0.4,
        community: 1,
        in_degree: 0,
        out_degree: 0,
        exported: kind != "File",
        is_dead_candidate: false,
        dead_reason: None,
        complexity: 1.0,
        is_test_file: false,
        test_count: 0,
        is_tested: false,
    }
}

fn seed(db: &GraphDb) {
    let nodes = vec![
        make_node(
            "file:crates/smoke/src/main.rs",
            "File",
            "crates/smoke/src/main.rs",
            "crates/smoke/src/main.rs",
        ),
        make_node(
            "fn:crates/smoke/src/main.rs:run",
            "Function",
            "run",
            "crates/smoke/src/main.rs",
        ),
        make_node(
            "file:crates/smoke/src/lib.rs",
            "File",
            "crates/smoke/src/lib.rs",
            "crates/smoke/src/lib.rs",
        ),
        make_node(
            "fn:crates/smoke/src/lib.rs:helper",
            "Function",
            "helper",
            "crates/smoke/src/lib.rs",
        ),
    ];
    let edges = vec![Edge {
        id: "fn:crates/smoke/src/main.rs:run|CALLS|fn:crates/smoke/src/lib.rs:helper".to_string(),
        src: "fn:crates/smoke/src/main.rs:run".to_string(),
        dst: "fn:crates/smoke/src/lib.rs:helper".to_string(),
        kind: "CALLS".to_string(),
        weight: 1.0,
        confidence: 1.0,
    }];
    db.upsert_nodes(&nodes).expect("upsert nodes");
    db.upsert_edges(&edges).expect("upsert edges");
    db.update_in_out_degrees().expect("update degrees");
}

#[test]
fn test_generate_vault_produces_expected_tree() {
    let repo = temp_repo_dir();
    let db = GraphDb::open(&repo).expect("open db");
    seed(&db);

    let out_dir = repo.join("cgx-docs");
    let opts = DocsOptions {
        prompt_packets: true,
        frontmatter: true,
        wiki_links_style: WikiLinkStyle::Obsidian,
        include_dead_code: true,
        include_duplicates: true,
    };
    let report = generate_vault(
        &repo,
        &db,
        DocsTarget::Local(out_dir.clone()),
        DocsMode::Full,
        &opts,
    )
    .expect("generate_vault");

    assert!(report.module_notes_written >= 1, "at least one module note");
    assert!(report.index_notes_written >= 5, "expected index notes");

    // Required top-level files / dirs.
    for path in [
        "README.md",
        "00-Overview/Architecture.md",
        "00-Overview/HowToNavigate.md",
        "00-Overview/Glossary.md",
        "20-Architecture/Groups.md",
        "20-Architecture/Communities.md",
        "20-Architecture/EntryPoints.md",
        "40-Risk/Hotspots.md",
        "50-Ownership/Owners.md",
    ] {
        let abs = out_dir.join(path);
        assert!(abs.exists(), "expected {} to exist", abs.display());
    }

    // Architecture overview should pull in the README excerpt + Cargo dep.
    let arch = std::fs::read_to_string(out_dir.join("00-Overview/Architecture.md"))
        .expect("read Architecture");
    assert!(
        arch.contains("smoke-test"),
        "Architecture should mention project name"
    );
    assert!(
        arch.contains("Rust workspace"),
        "Architecture should detect Rust stack"
    );
    assert!(
        arch.contains("serde"),
        "Architecture should list the serde dep"
    );

    // Module notes should carry a prose stub for the AI to fill in.
    let main_note = out_dir.join("30-Modules/smoke/main.rs.md");
    assert!(main_note.exists(), "expected module note for main.rs");
    let main_content = std::fs::read_to_string(&main_note).expect("read main note");
    assert!(
        main_content.contains("<!-- cgx-prompt:begin -->"),
        "module note should contain a cgx-prompt stub"
    );
    assert!(
        main_content.contains("## Structure"),
        "module note should have a Structure section"
    );

    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn test_incremental_skips_unchanged_files() {
    let repo = temp_repo_dir();
    let db = GraphDb::open(&repo).expect("open db");
    seed(&db);
    let out_dir = repo.join("cgx-docs");
    let opts = DocsOptions::default();

    let first = generate_vault(
        &repo,
        &db,
        DocsTarget::Local(out_dir.clone()),
        DocsMode::Full,
        &opts,
    )
    .expect("first generate");
    assert!(first.module_notes_written >= 1);

    let second = generate_vault(
        &repo,
        &db,
        DocsTarget::Local(out_dir.clone()),
        DocsMode::Incremental,
        &opts,
    )
    .expect("second generate");
    assert_eq!(
        second.module_notes_written, 0,
        "incremental run should skip every file when nothing changed"
    );
    assert!(
        second.module_notes_skipped >= 1,
        "should report skipped notes"
    );

    let _ = std::fs::remove_dir_all(&repo);
}
