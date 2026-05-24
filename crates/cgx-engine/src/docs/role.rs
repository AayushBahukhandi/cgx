//! Heuristic role classifier for a source file. Drives the TL;DR badge and
//! a frontmatter tag in each module note.
//!
//! Pure heuristics over path + language + node kinds — no AST inspection.

use crate::graph::FileSummary;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileRole {
    Test,
    Fixture,
    Cli,
    Parser,
    QueryLayer,
    Config,
    Build,
    Model,
    View,
    ApiServer,
    Mcp,
    Skill,
    Entry,
    Library,
    Other,
}

impl FileRole {
    pub fn label(self) -> &'static str {
        match self {
            FileRole::Test => "test",
            FileRole::Fixture => "fixture",
            FileRole::Cli => "cli",
            FileRole::Parser => "parser",
            FileRole::QueryLayer => "query",
            FileRole::Config => "config",
            FileRole::Build => "build",
            FileRole::Model => "model",
            FileRole::View => "view",
            FileRole::ApiServer => "api",
            FileRole::Mcp => "mcp",
            FileRole::Skill => "skill",
            FileRole::Entry => "entry",
            FileRole::Library => "library",
            FileRole::Other => "other",
        }
    }

    /// One-line description used in the TL;DR headline.
    pub fn description(self) -> &'static str {
        match self {
            FileRole::Test => "Test suite",
            FileRole::Fixture => "Test fixture",
            FileRole::Cli => "CLI command / argument plumbing",
            FileRole::Parser => "Language parser (Tree-sitter front-end)",
            FileRole::QueryLayer => "Graph query / data-access layer",
            FileRole::Config => "Configuration & settings",
            FileRole::Build => "Build / packaging glue",
            FileRole::Model => "Domain model / data types",
            FileRole::View => "UI component (renders to screen)",
            FileRole::ApiServer => "HTTP API server / handler",
            FileRole::Mcp => "MCP (Model Context Protocol) tool surface",
            FileRole::Skill => "AI agent skill / instructions",
            FileRole::Entry => "Program entry point",
            FileRole::Library => "General-purpose library code",
            FileRole::Other => "General module",
        }
    }
}

/// Classify a file using path + summary signals.
pub fn classify(summary: &FileSummary) -> FileRole {
    let path = summary.path.to_lowercase();

    // Tests & fixtures take priority — they're unambiguous.
    if path.contains("/tests/")
        || path.ends_with("_test.go")
        || path.contains(".test.")
        || path.ends_with("_test.rs")
        || path.ends_with("_test.py")
        || path.contains("/__tests__/")
    {
        return if path.contains("/fixtures/") || path.contains("/fixture/") {
            FileRole::Fixture
        } else {
            FileRole::Test
        };
    }
    if path.contains("/fixtures/") || path.contains("/fixture/") {
        return FileRole::Fixture;
    }

    // Entry points by convention.
    let basename = path.rsplit('/').next().unwrap_or(&path);
    if matches!(
        basename,
        "main.rs"
            | "main.go"
            | "main.py"
            | "main.ts"
            | "main.js"
            | "lib.rs"
            | "mod.rs"
            | "index.ts"
            | "index.js"
            | "index.tsx"
    ) {
        if basename == "lib.rs" || basename == "mod.rs" {
            return FileRole::Library;
        }
        return FileRole::Entry;
    }

    // Path-prefix-driven roles.
    if path.contains("/parsers/") || path.contains("/parser/") || path.ends_with("/parser.rs") {
        return FileRole::Parser;
    }
    if path.contains("/mcp/")
        || path.contains("mcp-server")
        || path.contains("/tools.rs") && path.contains("mcp")
    {
        return FileRole::Mcp;
    }
    if path.contains("/cli/") || path.contains("/cmd/") || path.contains("/commands/") {
        return FileRole::Cli;
    }
    if path.contains("/components/")
        || path.contains("/views/")
        || path.contains("/pages/")
        || path.ends_with(".tsx")
        || path.ends_with(".jsx")
    {
        return FileRole::View;
    }
    if path.contains("/api/")
        || path.contains("/server/")
        || path.contains("/handlers/")
        || path.contains("/routes/")
        || path.ends_with("/serve.rs")
        || path.ends_with("/server.rs")
    {
        return FileRole::ApiServer;
    }
    if path.contains("/config")
        || path.ends_with("config.rs")
        || path.ends_with("config.ts")
        || path.ends_with("settings.py")
        || path.ends_with("conf.py")
    {
        return FileRole::Config;
    }
    if path.contains("/models/") || path.contains("/model/") || path.contains("/types/") {
        return FileRole::Model;
    }
    if path.ends_with("graph.rs")
        || path.ends_with(".sql")
        || path.contains("/db/")
        || path.ends_with("queries.rs")
        || path.ends_with("repository.rs")
        || path.contains("/dao/")
    {
        return FileRole::QueryLayer;
    }
    if path.contains("/skill") || path.ends_with("skill.rs") {
        return FileRole::Skill;
    }
    if path.contains("build.rs")
        || path.contains("/scripts/")
        || path.contains("/build/")
        || path.contains(".github/")
    {
        return FileRole::Build;
    }

    // Signal-based fallback: lots of exported types and few cross-file callers → library.
    if summary.exported_count >= 8 && summary.callers.len() <= 3 {
        return FileRole::Library;
    }

    FileRole::Other
}
