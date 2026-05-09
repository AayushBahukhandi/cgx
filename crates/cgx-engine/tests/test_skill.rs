use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use cgx_engine::{
    build_skill_data, generate_agents_md, generate_skill, CommunityInfo, Edge, GraphDb, Node,
    SkillData,
};

static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

fn temp_dir() -> PathBuf {
    let count = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("cgx-skill-test-{}-{}", std::process::id(), count));
    std::fs::create_dir_all(&dir).expect("failed to create test dir");
    std::fs::write(dir.join("dummy.txt"), "test").expect("failed to write dummy file");
    dir
}

fn make_node(
    id: &str,
    kind: &str,
    name: &str,
    path: &str,
    language: &str,
    churn: f64,
    coupling: f64,
    community: i64,
    in_degree: i64,
    out_degree: i64,
) -> Node {
    Node {
        id: id.to_string(),
        kind: kind.to_string(),
        name: name.to_string(),
        path: path.to_string(),
        line_start: 1,
        line_end: 10,
        language: language.to_string(),
        churn,
        coupling,
        community,
        in_degree,
        out_degree,
        exported: false,
        is_dead_candidate: false,
        dead_reason: None,
    }
}

fn seed_graph(db: &GraphDb) {
    let nodes = vec![
        make_node(
            "fn:src/auth.ts:login",
            "Function",
            "login",
            "src/auth.ts",
            "typescript",
            0.8,
            0.5,
            1,
            2,
            1,
        ),
        make_node(
            "cls:src/auth.ts:AuthService",
            "Class",
            "AuthService",
            "src/auth.ts",
            "typescript",
            0.3,
            0.7,
            1,
            1,
            0,
        ),
        make_node(
            "fn:src/db.ts:query",
            "Function",
            "query",
            "src/db.ts",
            "typescript",
            0.0,
            0.2,
            2,
            0,
            0,
        ),
        make_node(
            "file:src/auth.ts",
            "File",
            "src/auth.ts",
            "src/auth.ts",
            "typescript",
            0.8,
            0.7,
            1,
            0,
            0,
        ),
        make_node(
            "file:src/db.ts",
            "File",
            "src/db.ts",
            "src/db.ts",
            "typescript",
            0.2,
            0.0,
            2,
            0,
            0,
        ),
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
    ];

    db.upsert_nodes(&nodes).expect("upsert nodes failed");
    db.upsert_edges(&edges).expect("upsert edges failed");
    db.update_in_out_degrees().expect("update degrees failed");
}

#[test]
fn test_build_skill_data_basic() {
    let dir = temp_dir();
    let db = GraphDb::open(&dir).expect("failed to open db");
    seed_graph(&db);

    let data = build_skill_data(&db).expect("build_skill_data failed");

    assert!(data.node_count > 0, "node_count should be > 0");
    assert!(data.edge_count > 0, "edge_count should be > 0");
    assert!(data.function_count > 0, "function_count should be > 0");
    assert!(data.file_count > 0, "file_count should be > 0");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_generate_skill_has_no_placeholders() {
    let data = SkillData {
        indexed_at: "2026-05-01T12:00:00Z".to_string(),
        node_count: 42,
        function_count: 10,
        class_count: 5,
        file_count: 8,
        edge_count: 100,
        language_breakdown: "TypeScript 100%".to_string(),
        community_count: 2,
        top_communities: vec![
            CommunityInfo {
                id: 1,
                label: "auth".to_string(),
                node_count: 20,
            },
            CommunityInfo {
                id: 2,
                label: "db".to_string(),
                node_count: 22,
            },
        ],
        hotspots: vec![],
        entry_points: vec![],
        god_nodes: vec![],
        dead_code_count: 0,
        dead_code_high: 0,
    };

    let skill = generate_skill(&data);
    let unfilled: Vec<_> = skill.match_indices("{{").collect();
    assert!(
        unfilled.is_empty(),
        "skill should have no unfilled placeholders: {:?}",
        unfilled
    );

    // Check required sections are present
    assert!(
        skill.contains("## When to Use cgx"),
        "missing 'When to Use cgx' section"
    );
    assert!(
        skill.contains("## Trigger Patterns"),
        "missing 'Trigger Patterns' section"
    );
    assert!(skill.contains("## Commands"), "missing 'Commands' section");
    assert!(skill.contains("## Workflow"), "missing 'Workflow' section");
    assert!(
        skill.contains("## Token Budget"),
        "missing 'Token Budget' section"
    );
    assert!(
        skill.contains("## This Codebase"),
        "missing 'This Codebase' section"
    );
    assert!(
        skill.contains("cgx summary"),
        "missing 'cgx summary' command"
    );
    assert!(
        skill.contains("cgx query find"),
        "missing 'cgx query find' command"
    );
    assert!(
        skill.contains("cgx query blast-radius"),
        "missing 'cgx query blast-radius' command"
    );
    assert!(
        skill.contains("cgx hotspots"),
        "missing 'cgx hotspots' command"
    );
}

#[test]
fn test_generate_skill_has_stats() {
    let data = SkillData {
        indexed_at: "2026-05-01T12:00:00Z".to_string(),
        node_count: 42,
        function_count: 10,
        class_count: 5,
        file_count: 8,
        edge_count: 100,
        language_breakdown: "TypeScript 100%".to_string(),
        community_count: 2,
        top_communities: vec![],
        hotspots: vec![],
        entry_points: vec![],
        god_nodes: vec![],
        dead_code_count: 0,
        dead_code_high: 0,
    };

    let skill = generate_skill(&data);
    assert!(skill.contains("42"), "skill should contain node count");
    assert!(skill.contains("100"), "skill should contain edge count");
    assert!(
        skill.contains("TypeScript 100%"),
        "skill should contain language breakdown"
    );
    assert!(
        skill.contains("2026-05-01T12:00:00Z"),
        "skill should contain indexed_at"
    );
}

#[test]
fn test_generate_agents_md_has_no_placeholders() {
    let data = SkillData {
        indexed_at: "2026-05-01T12:00:00Z".to_string(),
        node_count: 42,
        function_count: 10,
        class_count: 5,
        file_count: 8,
        edge_count: 100,
        language_breakdown: "TypeScript 100%".to_string(),
        community_count: 2,
        top_communities: vec![CommunityInfo {
            id: 1,
            label: "auth".to_string(),
            node_count: 20,
        }],
        hotspots: vec![],
        entry_points: vec![],
        god_nodes: vec![],
        dead_code_count: 0,
        dead_code_high: 0,
    };

    let agents = generate_agents_md(&data);
    let unfilled: Vec<_> = agents.match_indices("{{").collect();
    assert!(
        unfilled.is_empty(),
        "agents md should have no unfilled placeholders: {:?}",
        unfilled
    );

    assert!(agents.contains("## Overview"), "missing 'Overview' section");
    assert!(
        agents.contains("## Module Map"),
        "missing 'Module Map' section"
    );
    assert!(agents.contains("## Hotspots"), "missing 'Hotspots' section");
    assert!(
        agents.contains("## Entry Points"),
        "missing 'Entry Points' section"
    );
    assert!(
        agents.contains("CGX_SKILL.md"),
        "should mention CGX_SKILL.md"
    );
}

#[test]
fn test_generate_skill_hotspots_section() {
    let hotspot = make_node(
        "file:src/auth.ts",
        "File",
        "src/auth.ts",
        "src/auth.ts",
        "typescript",
        0.9,
        0.8,
        1,
        5,
        0,
    );

    let data = SkillData {
        indexed_at: "2026-05-01T12:00:00Z".to_string(),
        node_count: 10,
        function_count: 3,
        class_count: 1,
        file_count: 2,
        edge_count: 5,
        language_breakdown: "TypeScript 100%".to_string(),
        community_count: 1,
        top_communities: vec![],
        hotspots: vec![hotspot],
        entry_points: vec![],
        god_nodes: vec![],
        dead_code_count: 0,
        dead_code_high: 0,
    };

    let skill = generate_skill(&data);
    assert!(skill.contains("### Hotspots"), "missing Hotspots section");
    assert!(
        skill.contains("src/auth.ts"),
        "hotspots should mention auth.ts"
    );
    assert!(
        skill.contains("0.90"),
        "hotspots should contain churn value"
    );
}

#[test]
fn test_generate_skill_entry_points_section() {
    let entry = make_node(
        "fn:src/main.ts:main",
        "Function",
        "main",
        "src/main.ts",
        "typescript",
        0.0,
        0.0,
        1,
        0,
        3,
    );

    let data = SkillData {
        indexed_at: "2026-05-01T12:00:00Z".to_string(),
        node_count: 5,
        function_count: 2,
        class_count: 0,
        file_count: 1,
        edge_count: 3,
        language_breakdown: "TypeScript 100%".to_string(),
        community_count: 1,
        top_communities: vec![],
        hotspots: vec![],
        entry_points: vec![entry],
        god_nodes: vec![],
        dead_code_count: 0,
        dead_code_high: 0,
    };

    let skill = generate_skill(&data);
    assert!(
        skill.contains("### Entry Points"),
        "missing Entry Points section"
    );
    assert!(skill.contains("main"), "entry points should mention main");
}

#[test]
fn test_generate_skill_god_nodes_section() {
    let god = make_node(
        "fn:src/db.ts:query",
        "Function",
        "query",
        "src/db.ts",
        "typescript",
        0.0,
        0.0,
        1,
        10,
        0,
    );

    let data = SkillData {
        indexed_at: "2026-05-01T12:00:00Z".to_string(),
        node_count: 5,
        function_count: 2,
        class_count: 0,
        file_count: 1,
        edge_count: 10,
        language_breakdown: "TypeScript 100%".to_string(),
        community_count: 1,
        top_communities: vec![],
        hotspots: vec![],
        entry_points: vec![],
        god_nodes: vec![god],
        dead_code_count: 0,
        dead_code_high: 0,
    };

    let skill = generate_skill(&data);
    assert!(
        skill.contains("### Most Depended-On Nodes"),
        "missing God Nodes section"
    );
    assert!(skill.contains("query"), "god nodes should mention query");
    assert!(
        skill.contains("10 callers"),
        "god nodes should show caller count"
    );
}
