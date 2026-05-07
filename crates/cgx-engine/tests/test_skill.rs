use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use cgx_engine::{GraphDb, Node, Edge, build_skill_data, generate_skill, generate_agents_md, SkillData, CommunityInfo};

static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

fn temp_dir() -> PathBuf {
    let count = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "cgx-skill-test-{}-{}",
        std::process::id(),
        count
    ));
    std::fs::create_dir_all(&dir).expect("failed to create test dir");
    std::fs::write(dir.join("dummy.txt"), "test").expect("failed to write dummy file");
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
            CommunityInfo { id: 1, label: "auth".to_string(), node_count: 20 },
            CommunityInfo { id: 2, label: "db".to_string(), node_count: 22 },
        ],
        hotspots: vec![],
        entry_points: vec![],
        god_nodes: vec![],
    };

    let skill = generate_skill(&data);
    let unfilled: Vec<_> = skill.match_indices("{{").collect();
    assert!(unfilled.is_empty(), "skill should have no unfilled placeholders: {:?}", unfilled);

    // Check required sections are present
    assert!(skill.contains("## When to Use cgx"), "missing 'When to Use cgx' section");
    assert!(skill.contains("## Trigger Patterns"), "missing 'Trigger Patterns' section");
    assert!(skill.contains("## Commands"), "missing 'Commands' section");
    assert!(skill.contains("## Workflow"), "missing 'Workflow' section");
    assert!(skill.contains("## Token Budget"), "missing 'Token Budget' section");
    assert!(skill.contains("## This Codebase"), "missing 'This Codebase' section");
    assert!(skill.contains("cgx summary"), "missing 'cgx summary' command");
    assert!(skill.contains("cgx query find"), "missing 'cgx query find' command");
    assert!(skill.contains("cgx query blast-radius"), "missing 'cgx query blast-radius' command");
    assert!(skill.contains("cgx hotspots"), "missing 'cgx hotspots' command");
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
    };

    let skill = generate_skill(&data);
    assert!(skill.contains("42"), "skill should contain node count");
    assert!(skill.contains("100"), "skill should contain edge count");
    assert!(skill.contains("TypeScript 100%"), "skill should contain language breakdown");
    assert!(skill.contains("2026-05-01T12:00:00Z"), "skill should contain indexed_at");
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
        top_communities: vec![
            CommunityInfo { id: 1, label: "auth".to_string(), node_count: 20 },
        ],
        hotspots: vec![],
        entry_points: vec![],
        god_nodes: vec![],
    };

    let agents = generate_agents_md(&data);
    let unfilled: Vec<_> = agents.match_indices("{{").collect();
    assert!(unfilled.is_empty(), "agents md should have no unfilled placeholders: {:?}", unfilled);

    assert!(agents.contains("## Overview"), "missing 'Overview' section");
    assert!(agents.contains("## Module Map"), "missing 'Module Map' section");
    assert!(agents.contains("## Hotspots"), "missing 'Hotspots' section");
    assert!(agents.contains("## Entry Points"), "missing 'Entry Points' section");
    assert!(agents.contains("## AI Integration"), "missing 'AI Integration' section");
    assert!(agents.contains("CGX_SKILL.md"), "should mention CGX_SKILL.md");
}

#[test]
fn test_generate_skill_hotspots_section() {
    let hotspot = Node {
        id: "file:src/auth.ts".to_string(),
        kind: "File".to_string(),
        name: "src/auth.ts".to_string(),
        path: "src/auth.ts".to_string(),
        line_start: 1,
        line_end: 1,
        language: "typescript".to_string(),
        churn: 0.9,
        coupling: 0.8,
        community: 1,
        in_degree: 5,
        out_degree: 0,
    };

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
    };

    let skill = generate_skill(&data);
    assert!(skill.contains("### Hotspots"), "missing Hotspots section");
    assert!(skill.contains("src/auth.ts"), "hotspots should mention auth.ts");
    assert!(skill.contains("0.90"), "hotspots should contain churn value");
}

#[test]
fn test_generate_skill_entry_points_section() {
    let entry = Node {
        id: "fn:src/main.ts:main".to_string(),
        kind: "Function".to_string(),
        name: "main".to_string(),
        path: "src/main.ts".to_string(),
        line_start: 1,
        line_end: 3,
        language: "typescript".to_string(),
        churn: 0.0,
        coupling: 0.0,
        community: 1,
        in_degree: 0,
        out_degree: 3,
    };

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
    };

    let skill = generate_skill(&data);
    assert!(skill.contains("### Entry Points"), "missing Entry Points section");
    assert!(skill.contains("main"), "entry points should mention main");
}

#[test]
fn test_generate_skill_god_nodes_section() {
    let god = Node {
        id: "fn:src/db.ts:query".to_string(),
        kind: "Function".to_string(),
        name: "query".to_string(),
        path: "src/db.ts".to_string(),
        line_start: 1,
        line_end: 3,
        language: "typescript".to_string(),
        churn: 0.0,
        coupling: 0.0,
        community: 1,
        in_degree: 10,
        out_degree: 0,
    };

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
    };

    let skill = generate_skill(&data);
    assert!(skill.contains("### Most Depended-On Nodes"), "missing God Nodes section");
    assert!(skill.contains("query"), "god nodes should mention query");
    assert!(skill.contains("10 callers"), "god nodes should show caller count");
}
