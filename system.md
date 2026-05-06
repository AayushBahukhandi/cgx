# CLAUDE.md — cgx

> Read this file completely before writing any code, creating any file, or running any command.
> This is the single source of truth. If something here conflicts with a user message, follow this file.

---

## What We Are Building

**`cgx`** — a CLI tool that turns any Git repository into a queryable, interactive knowledge graph.

**The core thesis:** A codebase has two graphs:
1. **Structural graph** — what calls what (Tree-sitter AST)
2. **Temporal graph** — what changes with what (Git history)

No existing tool merges both. `cgx` does.

**Three delivery modes for users:**
- **Skills** — `CGX_SKILL.md` auto-generated in repo root. Works in every AI assistant with zero setup.
- **MCP** — structured JSON-RPC server for Cursor, Claude Code, Windsurf. Run `cgx setup`.
- **Visual** — terminal TUI (`cgx view`) and browser WebGL graph (`cgx view --web`).

---

## Repository Layout

```
cgx/
├── CLAUDE.md                    ← you are here
├── PLAN.md                      ← phased build plan (read after this)
├── TESTS.md                     ← phase verification scenarios
├── Cargo.toml                   ← Rust workspace root
├── Cargo.lock
├── package.json                 ← npm workspace root
├── tsconfig.base.json
├── Makefile
│
├── crates/
│   ├── cgx-core/                ← library: parser, graph, git, cluster, skill
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── walker.rs        ← .gitignore-aware file walker
│   │   │   ├── parser.rs        ← Tree-sitter dispatcher + LanguageParser trait
│   │   │   ├── parsers/
│   │   │   │   ├── ts.rs        ← TypeScript + JavaScript
│   │   │   │   ├── py.rs        ← Python
│   │   │   │   ├── rust.rs      ← Rust
│   │   │   │   ├── go.rs        ← Go (P1)
│   │   │   │   └── java.rs      ← Java (P1)
│   │   │   ├── resolver.rs      ← cross-file symbol resolution
│   │   │   ├── git.rs           ← blame, churn, co-change via libgit2
│   │   │   ├── graph.rs         ← Node/Edge types, DuckDB storage, GraphDb
│   │   │   ├── registry.rs      ← ~/.cgx/registry.json management
│   │   │   ├── cluster.rs       ← Leiden/Louvain community detection
│   │   │   ├── export.rs        ← JSON / GraphML / DOT / Mermaid / SVG
│   │   │   └── skill.rs         ← CGX_SKILL.md + AGENTS.md generator
│   │   └── Cargo.toml
│   │
│   ├── cgx-cli/                 ← binary: all user-facing commands
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── cmd/
│   │   │   │   ├── analyze.rs   ← parse + store + git + cluster + skill + hooks
│   │   │   │   ├── view.rs      ← TUI or browser
│   │   │   │   ├── query.rs     ← cgx query subcommands
│   │   │   │   ├── export.rs
│   │   │   │   ├── publish.rs   ← GitHub Pages
│   │   │   │   ├── hotspots.rs
│   │   │   │   ├── diff.rs
│   │   │   │   ├── setup.rs     ← editor MCP config writer
│   │   │   │   ├── serve.rs     ← HTTP API server
│   │   │   │   ├── doctor.rs    ← diagnostic checker
│   │   │   │   └── update.rs    ← grammar updater
│   │   │   └── tui/
│   │   │       ├── app.rs
│   │   │       ├── graph_widget.rs
│   │   │       └── layout.rs
│   │   └── Cargo.toml
│   │
│   └── cgx-mcp/                 ← binary: MCP stdio server
│       ├── src/
│       │   ├── main.rs
│       │   ├── server.rs        ← JSON-RPC 2.0 over stdio
│       │   └── tools.rs         ← 10 MCP tool implementations
│       └── Cargo.toml
│
├── packages/
│   └── web-ui/                  ← Vite + React + TypeScript
│       ├── src/
│       │   ├── main.tsx
│       │   ├── App.tsx
│       │   ├── components/
│       │   │   ├── GraphCanvas.tsx    ← Sigma.js WebGL renderer
│       │   │   ├── Sidebar.tsx        ← node inspector
│       │   │   ├── FilterBar.tsx      ← kind/community/edge filters
│       │   │   ├── SearchBar.tsx
│       │   │   ├── HotspotsPanel.tsx
│       │   │   └── CommandPalette.tsx ← Cmd+K search
│       │   ├── hooks/
│       │   │   ├── useGraph.ts
│       │   │   ├── useSearch.ts
│       │   │   └── useKeyboard.ts
│       │   └── types/
│       │       └── graph.ts           ← shared node/edge types
│       ├── index.html
│       ├── vite.config.ts
│       └── package.json
│
└── scripts/
    ├── integration-test.sh       ← full end-to-end test runner
    ├── bench.sh                  ← benchmark on large repos
    └── release.sh                ← version bump + tag
```

---

## Core Data Model

### Node Types
```
File       — every source file in the repo
Module     — logical grouping (folder / package)
Function   — function, method, arrow function
Class      — class, struct, interface, trait, enum
Variable   — exported top-level constants
Type       — type aliases, type parameters
Author     — git contributor (from blame)
```

### Edge Types
```
CALLS        — function A calls function B
IMPORTS      — file A imports from file B
INHERITS     — class A extends class B
IMPLEMENTS   — class A implements interface B
EXPORTS      — module A exports symbol B
CO_CHANGES   — file A and B changed in same commit (temporal)
OWNS         — author A is primary owner of file B (by blame %)
DEPENDS_ON   — file A depends on file B (derived from IMPORTS)
```

### Node ID Format
All node IDs follow this format: `<kind_prefix>:<relative_path>:<name>`
Examples:
- `fn:src/auth/service.ts:login`
- `cls:src/auth/service.ts:AuthService`
- `file:src/auth/service.ts`
- `author:alice@example.com`

This format must be consistent across all parsers and must never change
after a node is created (it is the primary key in DuckDB).

---

## DuckDB Schema

Every indexed repo gets its own DuckDB file at `~/.cgx/repos/<sha256-of-path>.db`.

```sql
CREATE TABLE nodes (
  id         VARCHAR PRIMARY KEY,
  kind       VARCHAR NOT NULL,
  name       VARCHAR NOT NULL,
  path       VARCHAR NOT NULL,
  line_start INTEGER,
  line_end   INTEGER,
  language   VARCHAR,
  churn      DOUBLE DEFAULT 0.0,
  coupling   DOUBLE DEFAULT 0.0,
  community  BIGINT DEFAULT 0,
  in_degree  BIGINT DEFAULT 0,
  out_degree BIGINT DEFAULT 0,
  metadata   JSON
);

CREATE TABLE edges (
  id         VARCHAR PRIMARY KEY,
  src        VARCHAR NOT NULL,
  dst        VARCHAR NOT NULL,
  kind       VARCHAR NOT NULL,
  weight     DOUBLE DEFAULT 1.0,
  confidence DOUBLE DEFAULT 1.0,
  metadata   JSON
);

CREATE TABLE communities (
  id         INTEGER PRIMARY KEY,
  label      VARCHAR,
  node_count INTEGER,
  top_nodes  JSON
);

CREATE TABLE repo_meta (
  key        VARCHAR PRIMARY KEY,
  value      JSON
);

CREATE INDEX idx_nodes_kind      ON nodes(kind);
CREATE INDEX idx_nodes_path      ON nodes(path);
CREATE INDEX idx_nodes_community ON nodes(community);
CREATE INDEX idx_edges_src       ON edges(src);
CREATE INDEX idx_edges_dst       ON edges(dst);
CREATE INDEX idx_edges_kind      ON edges(kind);
```

### Global Registry

`~/.cgx/registry.json` tracks all indexed repos:
```json
{
  "version": 1,
  "repos": [
    {
      "id": "sha256:abc123",
      "name": "my-project",
      "path": "/home/user/projects/my-project",
      "db_path": "~/.cgx/repos/abc123.db",
      "indexed_at": "2026-05-01T12:00:00Z",
      "node_count": 1203,
      "edge_count": 3847,
      "language_breakdown": { "typescript": 0.68, "python": 0.32 }
    }
  ]
}
```

---

## Key Rust Types

### LanguageParser trait
```rust
pub trait LanguageParser: Send + Sync {
    fn extensions(&self) -> &[&str];
    fn extract(&self, file: &SourceFile) -> Result<ParseResult>;
}

pub struct SourceFile {
    pub path: PathBuf,
    pub relative_path: String,
    pub language: Language,
    pub content: String,
    pub size_bytes: u64,
}

pub struct ParseResult {
    pub nodes: Vec<NodeDef>,
    pub edges: Vec<EdgeDef>,
}

pub struct NodeDef {
    pub id: String,
    pub kind: NodeKind,
    pub name: String,
    pub path: String,
    pub line_start: u32,
    pub line_end: u32,
    pub metadata: serde_json::Value,
}

pub struct EdgeDef {
    pub src: String,
    pub dst: String,
    pub kind: EdgeKind,
    pub weight: f64,
    pub confidence: f64,
}
```

### GraphDb
```rust
pub struct GraphDb {
    conn: duckdb::Connection,
    pub repo_id: String,
    pub db_path: PathBuf,
}

impl GraphDb {
    pub fn open(repo_path: &Path) -> Result<Self>;
    pub fn upsert_nodes(&self, nodes: &[Node]) -> Result<usize>;
    pub fn upsert_edges(&self, edges: &[Edge]) -> Result<usize>;
    pub fn get_node(&self, id: &str) -> Result<Option<Node>>;
    pub fn get_neighbors(&self, id: &str, depth: u8) -> Result<Vec<Node>>;
    pub fn get_all_nodes(&self) -> Result<Vec<Node>>;
    pub fn get_all_edges(&self) -> Result<Vec<Edge>>;
    pub fn get_stats(&self) -> Result<RepoStats>;
    pub fn get_hotspots(&self, limit: usize) -> Result<Vec<Node>>;
    pub fn get_entry_points(&self, limit: usize) -> Result<Vec<Node>>;
    pub fn get_god_nodes(&self, limit: usize) -> Result<Vec<Node>>;
    pub fn update_node_scores(&self) -> Result<()>;
}
```

### SkillData
```rust
pub struct SkillData {
    pub indexed_at: String,
    pub node_count: u64,
    pub function_count: u64,
    pub class_count: u64,
    pub file_count: u64,
    pub edge_count: u64,
    pub language_breakdown: String,
    pub community_count: u32,
    pub top_communities: Vec<CommunityInfo>,
    pub hotspots: Vec<Node>,
    pub entry_points: Vec<Node>,
    pub god_nodes: Vec<Node>,
}
```

---

## MCP Tools (10 total)

The MCP server speaks JSON-RPC 2.0 over **stdio only**.
**Debug output goes to stderr only — stdout is reserved for JSON-RPC.**

```
get_repo_summary    → full architectural overview (always call first in a session)
find_symbol         → locate any function/class by name + optional kind filter
get_neighbors       → direct dependencies of a node (depth 1-3)
get_call_chain      → trace call path between two symbols
get_blast_radius    → all nodes affected if X changes + risk level
get_community       → all nodes in a cluster
search_graph        → full-text search over names and paths
get_hotspots        → highest churn × coupling nodes
get_file_owners     → git blame ownership breakdown
run_query           → read-only SQL against DuckDB (SELECT only)
```

`run_query` must reject any SQL containing (case-insensitive):
`INSERT`, `UPDATE`, `DELETE`, `DROP`, `CREATE`, `ALTER`, `TRUNCATE`, `PRAGMA`

Return error: `{ "code": -32000, "message": "Only SELECT queries are permitted" }`

All tool errors return proper JSON-RPC error objects — never panic in MCP server code.

---

## Skills System

`cgx analyze` generates two files in the repo root automatically after every run.

**`CGX_SKILL.md`** — operational command reference for AI assistants.
Teaches the AI when to run `cgx` commands instead of reading files.
Contains live codebase stats from DuckDB (node counts, hotspots, communities).
Template uses `{{ placeholder }}` tokens — every token must be replaced before writing.
Never write a file containing unreplaced `{{ }}` tokens.

**`AGENTS.md`** — prose architecture summary for humans and AI agents.
Describes module structure, hotspots, entry points, and both integration modes.

Both files must be committed to the repo — they are living documentation.
Both files regenerate on every `cgx analyze` run.

### Git Hooks

At the end of `cgx analyze`, install these hooks if `.git/` exists:
```bash
#!/bin/sh
# cgx-managed
cgx analyze --incremental --quiet
```

Rules:
- Write `# cgx-managed` on **line 2** of every hook cgx creates
- If hook exists WITHOUT `# cgx-managed` on line 2: warn and skip, never overwrite
- Make hook files executable (`chmod +x` via `std::fs::Permissions`)
- `--incremental`: only re-parse files changed since last indexed commit
- `--quiet`: suppress all output except errors

---

## Languages Supported

| Language   | Grammar crate            | Priority |
|------------|--------------------------|----------|
| TypeScript | tree-sitter-typescript   | P0       |
| JavaScript | tree-sitter-javascript   | P0       |
| Python     | tree-sitter-python       | P0       |
| Rust       | tree-sitter-rust         | P0       |
| Go         | tree-sitter-go           | P1       |
| Java       | tree-sitter-java         | P1       |
| C / C++    | tree-sitter-c/cpp        | P1       |
| C#         | tree-sitter-c-sharp      | P2       |
| PHP        | tree-sitter-php          | P2       |
| Swift      | tree-sitter-swift        | P2       |

Build all P0 parsers before moving to P1. Each parser is independent
and registered via `ParserRegistry::new()` — adding one never breaks others.

---

## Full CLI Reference

```bash
# ANALYSIS
cgx analyze                        # index current repo
cgx analyze ./path                 # index any local path
cgx analyze github:owner/repo      # clone + index remote
cgx analyze --watch                # file-watch mode (notify crate)
cgx analyze --force                # full re-index
cgx analyze --incremental          # only re-parse changed files
cgx analyze --quiet                # errors only
cgx analyze --no-git               # skip git layer
cgx analyze --no-cluster           # skip clustering
cgx analyze --no-hooks             # skip git hook installation
cgx analyze --verbose              # show per-file parse results

# QUERY
cgx query find <name>              # find symbol by name
cgx query find <name> --kind=Class # filter by kind
cgx query deps <name>              # direct dependencies
cgx query blast-radius <name>      # downstream impact
cgx query chain "<A> -> <B>"       # trace call path
cgx query owners <path>            # git blame ownership
cgx query search "<phrase>"        # full-text search
cgx query community <id>           # nodes in a cluster
cgx query dead-code                # unreferenced exports

# VISUALIZATION
cgx view                           # terminal TUI
cgx view --web                     # browser WebGL graph
cgx view --filter=src/auth         # scope to subfolder
cgx view --community=3             # scope to cluster
cgx view --depth=2 UserService     # ego-graph around symbol

# GIT INTELLIGENCE
cgx hotspots                       # high churn x coupling
cgx hotspots --top=20
cgx blame-graph                    # ownership by contributor
cgx impact --since=7d              # changed nodes + ripple
cgx diff HEAD~5                    # graph diff between commits

# EXPORT
cgx export --format=json
cgx export --format=mermaid
cgx export --format=dot
cgx export --format=svg
cgx export --format=graphml
cgx export --out=./path

# GITHUB PAGES
cgx publish
cgx publish --dry-run
cgx publish --embed
cgx publish --badge

# REPO MANAGEMENT
cgx list
cgx status
cgx summary
cgx clean
cgx clean --all --force

# AI INTEGRATION
cgx mcp                            # start MCP server (stdio)
cgx serve                          # start HTTP API (port 7373)
cgx setup                          # write MCP config for editors
cgx setup --dry-run

# MAINTENANCE
cgx doctor                         # diagnose full setup
cgx update                         # update Tree-sitter grammars
cgx --version
```

---

## Cargo Dependencies

### cgx-core
```toml
[dependencies]
tree-sitter = "0.22"
tree-sitter-typescript = "0.21"
tree-sitter-javascript = "0.21"
tree-sitter-python = "0.21"
tree-sitter-rust = "0.21"
tree-sitter-go = "0.21"
tree-sitter-java = "0.21"
duckdb = { version = "0.10", features = ["bundled"] }
git2 = "0.18"
rayon = "1.10"
ignore = "0.4"
dirs = "5"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
thiserror = "1"
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
chrono = { version = "0.4", features = ["serde"] }
sha2 = "0.10"
fastrand = "2"
```

### cgx-cli
```toml
[dependencies]
cgx-core = { path = "../cgx-core" }
clap = { version = "4", features = ["derive", "env"] }
ratatui = "0.26"
crossterm = "0.27"
indicatif = "0.17"
console = "0.15"
open = "5"
axum = "0.7"
tower-http = { version = "0.5", features = ["fs", "cors"] }
notify = "6"
tokio = { version = "1", features = ["full"] }
```

### cgx-mcp
```toml
[dependencies]
cgx-core = { path = "../cgx-core" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
anyhow = "1"
```

### web-ui (package.json)
```json
{
  "dependencies": {
    "react": "^18",
    "react-dom": "^18",
    "sigma": "^3",
    "graphology": "^0.25",
    "graphology-layout-forceatlas2": "^0.10",
    "graphology-communities-louvain": "^2"
  },
  "devDependencies": {
    "vite": "^5",
    "typescript": "^5",
    "@types/react": "^18",
    "tailwindcss": "^3"
  }
}
```

---

## Web UI Design

Dark industrial / terminal aesthetic — non-negotiable:

- Background: `#0a0a0f`
- Panel background: `#111118`, borders `1px solid #1e1e2e`
- **Sharp corners everywhere — no border-radius**
- Fonts: `JetBrains Mono` for code labels; `Syne` for UI chrome
- Node colors: Function=`#00ff88`, Class=`#3b82f6`, File=`#f59e0b`, Module=`#8b5cf6`, Author=`#ec4899`
- Node size: `4 + (churn * 12)px`
- Edge opacity: CALLS=`#ffffff22`, CO_CHANGES=`#ef444466`, IMPORTS=`#3b82f644`
- Community hulls: convex polygon, `fill-opacity: 0.04`, `stroke-opacity: 0.3`
- High-churn nodes: `filter: drop-shadow(0 0 6px currentColor)`
- Cmd+K command palette for search and navigation
- ForceAtlas2 layout runs in Web Worker — never blocks the main thread

---

## TUI Layout (Ratatui)

```
┌─ Graph (60%) ───────────────────┬─ Inspector (40%) ──┐
│                                 │ ● UserService       │
│   [force-directed ASCII]        │ Kind:  Class        │
│                                 │ File:  src/user.rs  │
│   ◉ = selected                  │ Lines: 14-67        │
│                                 │ Churn: ████░░ 0.71  │
│                                 │ Coup:  ███░░░ 0.58  │
│                                 │ Comm:  #3 auth      │
│                                 │                     │
│                                 │ Callers (4)         │
│                                 │  AuthController     │
│                                 │  UserRouter         │
├─ Status ────────────────────────┴─────────────────────┤
│ 1,203 nodes · 3,847 edges · community 3/12            │
│ [q]uit [/]search [f]ilter [e]go [c]ommunity [?]help   │
└───────────────────────────────────────────────────────┘
```

---

## `cgx doctor` Output Format

```
cgx doctor

  Environment
  ✓ cgx binary:        /usr/local/bin/cgx  v0.1.0
  ✓ ~/.cgx/ directory: exists (14.2 MB)
  ✓ registry:          3 repos indexed

  Dependencies
  ✓ git:               detected (libgit2 bundled)
  ✗ graphviz dot:      not found (SVG export uses fallback)
  ✓ node / npm:        v20.11.0 / 10.2.4

  Editor Integrations
  ✓ Claude Code:       ~/.claude/settings.json  (cgx registered)
  ✓ Cursor:            ~/.cursor/mcp.json       (cgx registered)
  ✗ Windsurf:          not detected

  Indexed Repos
  ✓ my-project         1,203 nodes  indexed 2h ago
  ✗ old-project        path no longer exists (stale — run cgx clean)

  Skills
  ✓ my-project/CGX_SKILL.md   exists, no unfilled placeholders
  ✓ my-project/AGENTS.md      exists

  Git Hooks
  ✓ my-project  post-commit    cgx-managed, executable
  ✓ my-project  post-checkout  cgx-managed, executable
```

---

## Incremental Indexing (`--incremental`)

When `--incremental` is passed (auto-triggered by git hooks):
1. Read `last_indexed_commit` from `repo_meta` table
2. Get changed files via git2 diff between last commit and HEAD
3. Re-parse only those files
4. Delete existing nodes/edges for changed files from DuckDB
5. Insert new nodes/edges
6. Re-run resolver for affected files + their direct importers
7. Re-run clustering (always full — fast enough)
8. Regenerate `CGX_SKILL.md` and `AGENTS.md`
9. Update `last_indexed_commit` in `repo_meta`

If DB is missing or corrupt: fall back to full analyze with a warning.
Never full re-parse when `--incremental` is explicitly passed.

---

## Performance Requirements

| Repo size          | `cgx analyze` time | Memory |
|--------------------|-------------------|--------|
| Small  (<50 files) | < 1s              | < 50MB |
| Medium (500 files) | < 5s              | <200MB |
| Large (5000 files) | < 30s             | <500MB |

Use `rayon::par_iter` for all file parsing — never sequential.
Use DuckDB batch inserts — never one row at a time.
ForceAtlas2 runs in a Web Worker — never on the main thread.

---

## CI / GitHub Actions

### `.github/workflows/ci.yml` — runs on every push and PR
```yaml
name: CI
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --workspace
      - run: cargo clippy --workspace -- -D warnings -D clippy::unwrap_used
      - uses: actions/setup-node@v4
        with: { node-version: '20' }
      - run: npm ci && npm run build
```

### `.github/workflows/release.yml` — runs on `v*` tags
Builds binaries for:
- `x86_64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-pc-windows-msvc`

Attaches to GitHub Release. Naming: `cgx-v0.1.0-aarch64-apple-darwin.tar.gz`

---

## Distribution

- **Primary:** `cargo install cgx` (binary name: `cgx`)
- **Secondary:** Pre-built binaries on GitHub Releases for all 4 targets
- **Tertiary:** Homebrew formula after v1.0
- **Versioning:** SemVer. Use `cargo-release`. `BREAKING CHANGE` triggers major.

---

## Error Handling Rules

1. All library code uses `anyhow::Result<T>` with `.context("description")`
2. Custom errors use `thiserror::Error`
3. **`unwrap()` and `expect()` are banned in library code** — `?` only
4. `clippy::unwrap_used` and `clippy::panic` are denied in `cgx-core` and `cgx-mcp`
5. Per-file parse failures are warnings — log with `tracing::warn!` and continue
6. Git layer failures are warnings — skip gracefully if not a git repo
7. DuckDB errors are wrapped — never surface raw duckdb errors to the user
8. MCP tool errors return JSON-RPC error objects — never panic
9. `--verbose` enables `tracing::debug!`; default level is `warn`
10. All terminal output uses the `console` crate — never raw ANSI codes

---

## Testing

```
crates/cgx-core/tests/
  fixtures/
    ts-sample/       ← TypeScript, 4 files, known symbols
    py-sample/       ← Python, 3 files, known symbols
    rust-sample/     ← Rust, 3 files, known symbols
    git-sample/      ← Real git repo, 4 commits, 2 authors (see TESTS.md)
  test_parser.rs
  test_resolver.rs
  test_git.rs
  test_graph.rs
  test_cluster.rs
  test_skill.rs      ← placeholder replacement, section presence, accuracy
  test_export.rs

scripts/
  integration-test.sh
  bench.sh
```

Run: `cargo test --workspace`
Lint: `cargo clippy --workspace -- -D warnings -D clippy::unwrap_used`

---

## What NOT To Do

- **NO** Neo4j, KuzuDB, or any external database — DuckDB only
- **NO** LLM API calls in core features — LLM is strictly opt-in
- **NO** shelling out to `git` binary — use `git2` crate exclusively
- **NO** hardcoded paths — use `dirs` crate for `~/.cgx/`
- **NO** sequential file parsing — always `rayon::par_iter`
- **NO** monolithic binary — keep `cgx-core`, `cgx-cli`, `cgx-mcp` separate
- **NO** `unwrap()` or `expect()` in library code
- **NO** stdout output from MCP server — stderr only for debug
- **NO** overwriting git hooks that lack the `# cgx-managed` marker
- **NO** full re-parse when `--incremental` is passed
- **NO** skipping `.gitignore` rules — use `ignore` crate everywhere
- **NO** absolute paths in DuckDB — all paths must be repo-relative
- **NO** unreplaced `{{ placeholder }}` tokens in generated skill files

---

## Phase Completion Criteria

A phase is complete only when ALL six pass:

1. All commands in that phase run without error on fixture repos
2. `cargo test --workspace` — **0 failures**
3. `cargo clippy --workspace -- -D warnings -D clippy::unwrap_used` — **0 errors**
4. All TESTS.md scenarios for that phase print `PASS`
5. No `unwrap()`, `expect()`, `todo!()`, `panic!()` in library code
6. Visual output verified where applicable (TUI renders, HTML opens, JSON valid)

Do not start Phase N+1 until Phase N passes all six criteria.
