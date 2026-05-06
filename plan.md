# PLAN.md — cgx Build Plan

> Paste this into Claude Code after CLAUDE.md is in the repo root.
> Work through phases IN ORDER. Do not start Phase N+1 until Phase N passes all tests.
> Each phase ends with a working, testable artifact.

---

## Phase 0 — Scaffold the Workspace

**Goal:** Get a compiling Rust workspace + TypeScript package layout with zero functionality yet.

### Tasks

1. Create `Cargo.toml` (workspace root):
```toml
[workspace]
members = ["crates/cgx-core", "crates/cgx-cli", "crates/cgx-mcp"]
resolver = "2"

[workspace.dependencies]
anyhow = "1"
thiserror = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
rayon = "1.10"
tree-sitter = "0.22"
duckdb = { version = "0.10", features = ["bundled"] }
git2 = "0.18"
ignore = "0.4"
dirs = "5"
chrono = { version = "0.4", features = ["serde"] }
sha2 = "0.10"
```

2. Create all three crates with `cargo new --lib` / `cargo new --bin`:
   - `crates/cgx-core` — lib
   - `crates/cgx-cli` — bin
   - `crates/cgx-mcp` — bin

3. Create `packages/web-ui` with `npm create vite@latest`:
   - Template: React + TypeScript
    - Install: `sigma`, `graphology`, `graphology-layout-forceatlas2`, `graphology-communities-louvain`, `tailwindcss`

4. Add root `package.json` as a workspace with:
```json
{
  "workspaces": ["packages/*"],
  "scripts": {
    "dev": "npm run dev --workspace=packages/web-ui",
    "build": "npm run build --workspace=packages/web-ui"
  }
}
```

5. Create `.gitignore`, `README.md` (minimal), and `Makefile`:
```makefile
build:
	cargo build --workspace
	npm run build

test:
	cargo test --workspace

dev-ui:
	npm run dev --workspace=packages/web-ui
```

### Done When
- `cargo build --workspace` compiles with zero errors
- `npm run build` produces a dist folder
- Directory tree matches layout in CLAUDE.md

---

## Phase 1 — File Walker + Tree-sitter Parsing (TypeScript + Python + Rust)

**Goal:** Given a repo path, walk all files, parse TS/JS/Python/Rust, extract
functions/classes/imports, and print structured JSON to stdout.

### Tasks

#### 1.1 — File Walker (`crates/cgx-core/src/walker.rs`)

Implement `walk_repo(path: &Path) -> Result<Vec<SourceFile>>` that:
- Uses the `ignore` crate to respect `.gitignore` and skip `node_modules`, `.git`, `target`, `dist`, `__pycache__`
- Returns a `SourceFile` struct: `{ path, language, content, size_bytes }`
- Detects language from file extension using a static lookup table
- Skips files over 2MB
- Skips binary files (check first 8kb for null bytes)

```rust
pub struct SourceFile {
    pub path: PathBuf,
    pub relative_path: String,
    pub language: Language,
    pub content: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Language {
    TypeScript, JavaScript, Python, Rust, Go, Java, CSharp, Unknown
}
```

#### 1.2 — Parser Trait (`crates/cgx-core/src/parser.rs`)

Define the `LanguageParser` trait:
```rust
pub trait LanguageParser: Send + Sync {
    fn extensions(&self) -> &[&str];
    fn extract(&self, file: &SourceFile) -> Result<ParseResult>;
}

pub struct ParseResult {
    pub nodes: Vec<NodeDef>,
    pub edges: Vec<EdgeDef>,
}

pub struct NodeDef {
    pub id: String,           // format: "<kind>:<rel_path>:<name>"
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

#### 1.3 — Language Parsers

Create one file per language in `crates/cgx-core/src/parsers/`:

**TypeScript / JavaScript** (`ts.rs`):
- Extract: functions (function decl, arrow fn, method), classes, interfaces, imports (`import ... from`), exports
- Use `tree-sitter-typescript` grammar
- Handle `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`
- Query patterns to extract (use Tree-sitter S-expression queries):
  - `(function_declaration name: (identifier) @name)`
  - `(class_declaration name: (type_identifier) @name)`
  - `(import_statement source: (string) @source)`
  - `(method_definition name: (property_identifier) @name)`

**Python** (`py.rs`):
- Extract: `def` functions, `class` definitions, `import` / `from X import Y`
- Use `tree-sitter-python` grammar

**Rust** (`rust.rs`):
- Extract: `fn` items, `struct` / `enum` / `trait` / `impl` blocks, `use` statements
- Use `tree-sitter-rust` grammar

#### 1.4 — Parser Dispatcher (`crates/cgx-core/src/parser.rs`)

```rust
pub struct ParserRegistry {
    parsers: HashMap<Language, Box<dyn LanguageParser>>,
}

impl ParserRegistry {
    pub fn new() -> Self { /* register all parsers */ }
    pub fn parse(&self, file: &SourceFile) -> Result<ParseResult> { /* dispatch */ }
    pub fn parse_all(&self, files: &[SourceFile]) -> Vec<ParseResult> {
        // use rayon::par_iter for parallelism
    }
}
```

#### 1.5 — Smoke Test Command

In `cgx-cli`, add `cgx parse <path>` (dev-only subcommand) that:
- Walks the repo
- Parses all files
- Prints summary: `Parsed 247 files: 1,203 functions, 89 classes, 412 imports`
- With `--json` flag: dumps full `ParseResult` as JSON to stdout

### Tests to Write
- `test_parser.rs`: fixture repos in `tests/fixtures/ts-sample/`, `py-sample/`, `rust-sample/`
- Each fixture has 3-5 files with known symbols
- Assert extracted node names and counts match expectations

### Done When
- `cgx parse ./` on a TypeScript project extracts correct symbols
- `cargo test --workspace` passes all parser tests
- No panics on any file in the fixture repos

---

## Phase 2 — Graph Builder + DuckDB Storage

**Goal:** Store parsed nodes/edges in DuckDB, support basic querying.

### Tasks

#### 2.1 — DuckDB Schema (`crates/cgx-core/src/graph.rs`)

Initialize the database at `~/.cgx/repos/<repo-hash>.db`.
Use the `dirs` crate to resolve `~/.cgx/`.
Create the schema defined in CLAUDE.md (nodes, edges, repo_meta tables).
Add `UNIQUE` constraints and `ON CONFLICT REPLACE` for upserts.

```rust
pub struct GraphDb {
    conn: duckdb::Connection,
    repo_id: String,
    db_path: PathBuf,
}

impl GraphDb {
    pub fn open(repo_path: &Path) -> Result<Self>;
    pub fn upsert_nodes(&self, nodes: &[Node]) -> Result<usize>;
    pub fn upsert_edges(&self, edges: &[Edge]) -> Result<usize>;
    pub fn get_node(&self, id: &str) -> Result<Option<Node>>;
    pub fn get_neighbors(&self, id: &str, depth: u8) -> Result<Vec<Node>>;
    pub fn get_all_nodes(&self) -> Result<Vec<Node>>;
    pub fn get_all_edges(&self) -> Result<Vec<Edge>>;
    pub fn node_count(&self) -> Result<u64>;
    pub fn edge_count(&self) -> Result<u64>;
}
```

#### 2.2 — Registry (`crates/cgx-core/src/registry.rs`)

Manage `~/.cgx/registry.json`:
```rust
pub struct Registry {
    pub version: u32,
    pub repos: Vec<RepoEntry>,
}
pub struct RepoEntry {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub db_path: PathBuf,
    pub indexed_at: String,
    pub node_count: u64,
    pub edge_count: u64,
    pub language_breakdown: HashMap<String, f64>,
}

impl Registry {
    pub fn load() -> Result<Self>;
    pub fn save(&self) -> Result<()>;
    pub fn register(&mut self, entry: RepoEntry);
    pub fn find_by_path(&self, path: &Path) -> Option<&RepoEntry>;
}
```

#### 2.3 — Cross-File Symbol Resolution (`crates/cgx-core/src/resolver.rs`)

After all files are parsed, run a second pass to resolve import edges:
- For each `import X from './foo'` edge, find the actual `File` node at that path
- Create `IMPORTS` edges between files
- For each `CALLS` edge where `dst` is an unresolved name, search all nodes for a matching name
  and create the edge if found (mark as `confidence: 0.8` if matched by name only)
- Build an index: `HashMap<String, Vec<String>>` mapping exported names to node IDs

```rust
pub fn resolve(nodes: &[NodeDef], edges: &[EdgeDef], repo_root: &Path) -> Vec<EdgeDef>;
```

#### 2.4 — `cgx analyze` command

Implement the full `cgx analyze [path]` flow:
1. Walk files (Phase 1)
2. Parse all files in parallel with rayon (Phase 1)
3. Resolve cross-file symbols (Phase 2.3)
4. Store nodes + edges in DuckDB (Phase 2.1)
5. Register repo in registry (Phase 2.2)
6. Print summary with `indicatif` progress bar:
```
  ✓ Walking files...             412 files found
  ✓ Parsing (parallel)...        1,203 nodes, 3,847 edges
  ✓ Resolving imports...         892 cross-file links resolved
  ✓ Storing graph...             saved to ~/.cgx/repos/abc123.db
  ✓ Done in 1.4s

  cgx view      — explore in terminal
  cgx view --web — explore in browser
```

#### 2.5 — `cgx status` and `cgx list` commands

`cgx status` — reads current dir's registry entry, prints node/edge counts, indexed_at, language breakdown
`cgx list` — prints table of all indexed repos from registry

### Tests to Write
- `test_graph.rs`: insert nodes/edges, query back, verify round-trip
- `test_resolver.rs`: fixture with two files where file A imports from file B — verify edge is created

### Done When
- `cgx analyze` runs on any TS or Python repo without errors
- DuckDB file is created and contains correct counts
- `cgx status` prints accurate info
- `cgx list` shows registered repos

---

## Phase 3 — Git Intelligence Layer

**Goal:** Add git blame (ownership), churn scoring, and co-change graph to every node.

### Tasks

#### 3.1 — Git Analyzer (`crates/cgx-core/src/git.rs`)

Use `git2` crate. Do NOT shell out to `git`.

```rust
pub struct GitAnalysis {
    pub file_churn: HashMap<PathBuf, f64>,      // normalized 0-1
    pub file_owners: HashMap<PathBuf, Vec<(String, f64)>>, // author → blame %
    pub co_changes: Vec<(PathBuf, PathBuf, u32)>, // (fileA, fileB, co_commit_count)
}

pub fn analyze_repo(repo_path: &Path) -> Result<GitAnalysis>;
```

**Churn score:** For each file, count commits that touched it in the last 90 days.
Normalize: `churn = count / max_count_in_repo`. Store on File nodes.

**Ownership (blame):** For each file, run git blame. Group lines by author email.
Ownership % = lines_by_author / total_lines. Store top 3 authors per file.

**Co-change graph:** Walk all commits. For each commit, collect the set of files changed.
For every pair (A, B) in that set, increment `co_changes[(A,B)]`.
After all commits, create `CO_CHANGES` edges where count >= 2.
Weight = count / max_count (normalized).

#### 3.2 — Integrate Git Layer into `cgx analyze`

After Phase 2 parsing/storage, run `analyze_repo()` and:
- Update File node `churn` field in DuckDB
- Insert `CO_CHANGES` edges into DuckDB
- Insert `OWNS` edges (Author node → File node) into DuckDB
- Add Author nodes for each contributor

Handle gracefully: if path is not a git repo, skip this phase with a warning.

#### 3.3 — `cgx hotspots` command

Query DuckDB for files where both `churn > 0.6` AND `coupling > 0.6` (high in-degree).
Print a ranked table:
```
  HOTSPOTS — high churn × high coupling
  ──────────────────────────────────────
  #  File                    Churn  Coupling  Callers
  1  src/auth/service.ts     0.92   0.78      14
  2  src/db/pool.ts          0.85   0.71       9
  3  src/api/router.ts       0.71   0.65      11
```

#### 3.4 — `cgx blame-graph` command

Print ownership by contributor:
```
  OWNERSHIP MAP
  ─────────────────────────────────────────────
  alice@dev.io   ████████████░░░░  63%  (142 files)
  bob@dev.io     ████░░░░░░░░░░░░  21%   (47 files)
  carol@dev.io   ██░░░░░░░░░░░░░░  16%   (36 files)
```

### Tests to Write
- `test_git.rs`: Use a fixture git repo with known commits, verify churn scores match expectations
- Test that co-change edges are created for files modified together

### Done When
- `cgx analyze` on a git repo populates churn scores and co-change edges
- `cgx hotspots` prints a meaningful ranked list
- Non-git folders are handled without crashing

---

## Phase 4 — Leiden Clustering

**Goal:** Group nodes into communities and label each cluster.

### Tasks

#### 4.1 — Leiden Implementation (`crates/cgx-core/src/cluster.rs`)

Implement Leiden community detection from scratch in Rust OR use a well-tested crate.
Input: adjacency list from DuckDB (all edges with weights).
Output: `HashMap<String, u32>` mapping node_id → community_id.

If implementing from scratch is too complex, use the **Louvain** algorithm instead
(simpler, produces similar results for this use case).

```rust
pub fn detect_communities(
    nodes: &[Node],
    edges: &[Edge],
) -> Result<HashMap<String, u32>>;
```

After detection, update every node's `community` field in DuckDB.
Compute community labels: find the most common `kind` + top-degree node name per community.
Store community metadata in `repo_meta` table.

#### 4.2 — Integrate clustering into `cgx analyze`

Add clustering as the final step of `cgx analyze`, after git layer.
Show progress: `✓ Clustering...   12 communities detected`

#### 4.3 — `cgx view --community=N` filter

Add community filter support: only show nodes and edges within community N.

### Done When
- Every node has a `community` integer after analysis
- Communities are stable across re-runs on the same repo
- `cgx view --community=3` scopes output correctly

---

## Phase 5 — Export Formats

**Goal:** Export the graph in multiple formats for downstream tools.

### Tasks

#### 5.1 — JSON export (`crates/cgx-core/src/export.rs`)

Implement `export_json(db: &GraphDb) -> Result<String>`.
Output matches the schema defined in CLAUDE.md exactly.
Write to `./cgx-graph.json` by default.

#### 5.2 — Mermaid export

Implement `export_mermaid(db: &GraphDb, max_nodes: usize) -> Result<String>`.
Cap at 100 nodes for readability. Prefer high-degree nodes.
Output:
```
graph TD
  A["UserService"] -->|CALLS| B["db.query"]
  C["AuthController"] -->|CALLS| A
```

#### 5.3 — DOT (Graphviz) export

Implement `export_dot(db: &GraphDb) -> Result<String>`.
Node color by `kind`, size by `churn`.

#### 5.4 — SVG export

Run `dot -Tsvg` on the DOT output if Graphviz is installed.
If Graphviz is not installed, fall back to a simple SVG using hard-coded
hierarchical layout logic (no external dependency required).

#### 5.5 — GraphML export

Implement `export_graphml(db: &GraphDb) -> Result<String>`.
Compatible with Gephi and yEd import.

#### 5.6 — `cgx export` command

```
cgx export --format=json --out=./graph.json
cgx export --format=mermaid
cgx export --format=dot --out=./graph.dot
cgx export --format=svg --out=./graph.svg
cgx export --format=graphml --out=./graph.graphml
```

### Done When
- All 5 formats produce valid output verified by hand or a parser
- JSON export round-trips: can be re-imported and matches original DuckDB state

---

## Phase 6 — Terminal TUI (Ratatui)

**Goal:** `cgx view` launches a force-directed graph in the terminal.

### Tasks

#### 6.1 — TUI App State (`crates/cgx-cli/src/tui/app.rs`)

```rust
pub struct App {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub selected: Option<String>,    // selected node id
    pub filter_community: Option<u32>,
    pub search_query: String,
    pub mode: AppMode,               // Normal | Search | Filter
    pub positions: HashMap<String, (f64, f64)>, // force layout positions
}
```

#### 6.2 — Force-Directed Layout

Implement a simple force-directed layout in `tui/layout.rs`:
- Repulsion between all nodes (inverse-square law)
- Attraction along edges (spring force)
- Run 100 iterations on startup; continue iterating while app is open
- Map positions to terminal cell coordinates

#### 6.3 — Graph Widget (`crates/cgx-cli/src/tui/graph_widget.rs`)

Implement a Ratatui `Widget` that renders:
- Nodes as colored Unicode blocks `●` — color by kind
- Edges as ASCII lines `─ │ ╱ ╲` between node positions
- Selected node highlighted with a bright border glyph `◉`
- Label text next to each node (truncated to available space)

#### 6.4 — Inspector Panel

Right panel shows selected node details:
- Name, kind, file path, line range
- Churn bar: `████░░ 0.71`
- Coupling bar (in-degree / max in-degree)
- Community ID
- Callers list (up to 8, scrollable)
- Callees list (up to 8, scrollable)

#### 6.5 — Keybindings

```
q / Esc    quit
/          enter search mode — filter nodes by name
f          open community filter picker
e          expand selected node (show 1-hop neighbors only)
Tab        cycle through nodes
Enter      select node under cursor
?          show keybinding help overlay
r          reset layout
```

#### 6.6 — `cgx view` command

Load graph from DuckDB, initialize App state, run Ratatui event loop.
Support `--filter=<path>` to scope to files under a subfolder.
Support `--community=<n>` to scope to a cluster.

### Done When
- `cgx view` renders without crashing on a real repo
- Navigation works with keyboard
- Inspector panel shows correct node info
- Terminal resizing is handled gracefully

---

## Phase 7 — Web UI (React + Sigma.js)

**Goal:** `cgx view --web` opens a browser with an interactive WebGL graph.

### Tasks

#### 7.1 — Graph Types (`packages/web-ui/src/types/graph.ts`)

Define TypeScript interfaces that match the JSON export format exactly:
```typescript
export interface GraphData {
  meta: RepoMeta;
  nodes: GraphNode[];
  edges: GraphEdge[];
  communities: Community[];
}

export interface GraphNode {
  id: string;
  kind: 'File' | 'Function' | 'Class' | 'Variable' | 'Type' | 'Module' | 'Author';
  name: string;
  path: string;
  churn: number;
  coupling: number;
  community: number;
}

export interface GraphEdge {
  src: string;
  dst: string;
  kind: 'CALLS' | 'IMPORTS' | 'INHERITS' | 'CO_CHANGES' | 'OWNS';
  weight: number;
}
```

#### 7.2 — Data Loading (`packages/web-ui/src/hooks/useGraph.ts`)

Load graph data from:
1. `window.__CGX_GRAPH__` if present (baked-in mode for `cgx publish`)
2. `http://localhost:7373/api/graph` if running against `cgx serve`
3. A dropped JSON file (drag-and-drop support)

#### 7.3 — Sigma.js Canvas (`packages/web-ui/src/components/GraphCanvas.tsx`)

Use `graphology` as the underlying graph data structure.
Use `sigma` for WebGL rendering.
Use `graphology-layout-forceatlas2` for layout (run in a Web Worker).

Node rendering:
- Size: `4 + (node.churn * 12)` — hotter = bigger
- Color by kind: `Function=#00ff88, Class=#3b82f6, File=#f59e0b, Module=#8b5cf6`
- On hover: show tooltip with name + kind + churn
- On click: update selected node → Inspector panel

Edge rendering:
- Width: `0.5 + edge.weight * 2`
- Color: `CALLS=#ffffff33, CO_CHANGES=#ef444455, IMPORTS=#3b82f655`

#### 7.4 — Sidebar / Inspector (`packages/web-ui/src/components/Sidebar.tsx`)

Selected node info panel (right side):
- Name, kind badge, file path
- Churn meter (colored bar)
- Community badge with cluster name
- Callers / Callees lists (clickable — navigate to that node)
- "Copy ID" button

#### 7.5 — Filter Bar (`packages/web-ui/src/components/FilterBar.tsx`)

Top bar with:
- Search input (filter nodes by name substring)
- Kind checkboxes (show/hide Functions, Classes, Files, etc.)
- Community dropdown (scope to a cluster)
- Edge type toggles (show/hide CALLS, CO_CHANGES, IMPORTS)

#### 7.6 — Visual Design

Follow the design direction in CLAUDE.md exactly:
- Background `#0a0a0f`, panels `#111118` with `1px solid #1e1e2e` borders
- Font: `JetBrains Mono` from Google Fonts for code labels, `Syne` for UI
- No rounded corners on panels (sharp `border-radius: 0`)
- Community hulls: render convex hull per cluster as a semi-transparent filled polygon
- Node glow effect: CSS filter `drop-shadow` on high-churn nodes
- Loading state: animated graph skeleton while ForceAtlas2 runs

#### 7.7 — `cgx serve` command (Rust)

Start an HTTP server on `localhost:7373` using `axum`:
```
GET /api/graph          — return full graph JSON for current indexed repo
GET /api/repos          — list all indexed repos (from registry)
GET /api/repos/:id/graph — get graph for specific repo
GET /                   — serve web UI static files
```

The server opens the browser automatically on start.

#### 7.8 — `cgx view --web` command

Run `cgx serve` in a background thread and open `http://localhost:7373` in the default browser.

### Done When
- `cgx view --web` opens browser with rendered graph
- Force layout runs and stabilizes
- Node click shows correct inspector data
- Filters work without page reload
- Works with graph JSON files up to 50MB

---

## Phase 8 — MCP Server + Skills System

**Goal:** Two parallel AI integration layers built together:
1. **MCP server** — typed tool protocol for Cursor, Claude Code, Windsurf
2. **Skill file** — zero-config markdown that works in ANY AI assistant without setup

Both are generated/activated by `cgx analyze`. Neither requires the other.

---

### Part A — MCP Server

#### 8.1 — MCP Protocol (`crates/cgx-mcp/src/server.rs`)

Implement JSON-RPC 2.0 over stdio.
Read newline-delimited JSON from `stdin`, write responses to `stdout`.
ALL debug/trace output goes to `stderr` only — stdout is the MCP stream and
any non-JSON written there will corrupt it and break editor integration.

Handle these MCP lifecycle messages:
```
initialize       → return server name, version, capabilities
initialized      → acknowledge (no response needed)
tools/list       → return array of all tool definitions with JSON Schema
tools/call       → dispatch name + arguments to the correct handler
ping             → respond with empty result (keep-alive)
```

Server info to return on initialize:
```json
{
  "protocolVersion": "2024-11-05",
  "capabilities": {
    "tools": {}
  },
  "serverInfo": {
    "name": "cgx",
    "version": "0.1.0"
  }
}
```

#### 8.2 — Tool Implementations (`crates/cgx-mcp/src/tools.rs`)

Implement all 10 tools. Each tool has:
- A JSON Schema definition (for `tools/list`)
- A handler function: `fn handle(args: Value, db: &GraphDb) -> Result<Value>`
- Response always wrapped as: `{ "content": [{ "type": "text", "text": "<json string>" }] }`

```rust
// Tool 1 — session bootstrap (always call this first)
get_repo_summary() -> {
  node_count: u64,
  edge_count: u64,
  languages: HashMap<String, f64>,     // { "typescript": 0.68, "python": 0.32 }
  communities: [{ id, label, node_count, top_nodes: [name] }],
  hotspots: Node[],        // top 5 by churn × coupling
  entry_points: Node[],    // nodes with 0 in-degree (nothing imports them)
  god_nodes: Node[],       // top 5 by in-degree (most depended-on)
  indexed_at: String
}

// Tool 2 — find any symbol by name
find_symbol(name: String, kind?: String) -> { nodes: Node[] }
// kind filter: "Function" | "Class" | "File" | "Type" | "Variable"

// Tool 3 — get direct dependencies of a node
get_neighbors(node_id: String, depth?: u8) -> { nodes: Node[], edges: Edge[] }
// depth defaults to 1, max 3

// Tool 4 — trace call path between two symbols
get_call_chain(from: String, to: String) -> { path: Node[], edges: Edge[], found: bool }

// Tool 5 — everything affected if this node changes
get_blast_radius(node_id: String) -> { affected: Node[], edge_count: u32, risk: String }
// risk: "LOW" | "MEDIUM" | "HIGH" | "CRITICAL" based on affected count

// Tool 6 — all nodes in a community cluster
get_community(community_id: u32) -> { nodes: Node[], label: String, edge_count: u32 }

// Tool 7 — full-text search over node names and paths
search_graph(query: String, limit?: u32) -> { nodes: Node[] }
// limit defaults to 20

// Tool 8 — highest risk files (churn × coupling)
get_hotspots(top_n?: u32) -> { nodes: HotspotNode[] }
// HotspotNode adds: churn, coupling, caller_count, owner

// Tool 9 — git blame ownership for a file
get_file_owners(file_path: String) -> { owners: [{ name, email, pct: f64, lines: u32 }] }

// Tool 10 — raw read-only SQL against DuckDB
run_query(sql: String) -> { rows: Vec<Value>, columns: Vec<String> }
// BLOCK any SQL containing: INSERT UPDATE DELETE DROP CREATE ALTER TRUNCATE PRAGMA
// Return error: "Only SELECT queries are permitted"
```

All handler errors must return a proper JSON-RPC error object, never panic:
```json
{ "code": -32000, "message": "Node not found: xyz", "data": null }
```

#### 8.3 — `cgx setup` command (`crates/cgx-cli/src/cmd/setup.rs`)

Detect installed editors by checking these paths (check all, not just first found):

```rust
struct EditorConfig {
    name: &'static str,
    config_dirs: &'static [&'static str],   // check all of these
    mcp_config_file: &'static str,
    mcp_json_path: &'static str,            // JSON pointer to mcpServers key
}

// Editors to detect:
// Claude Code: ~/.claude/settings.json  (key: mcpServers)
// Cursor:      ~/.cursor/mcp.json       (key: mcpServers)
// VS Code:     ~/.vscode/settings.json  (key: mcp.servers)  [if MCP ext installed]
// Windsurf:    ~/.windsurf/mcp.json     (key: mcpServers)
// Zed:         ~/.config/zed/settings.json (key: context_servers)
```

For each detected editor, merge (not overwrite) the MCP entry:
```json
{
  "mcpServers": {
    "cgx": {
      "command": "cgx",
      "args": ["mcp"],
      "env": {}
    }
  }
}
```

Print a summary:
```
  cgx setup — configuring AI editor integrations

  ✓ Claude Code  ~/.claude/settings.json
  ✓ Cursor       ~/.cursor/mcp.json
  ✗ VS Code      not detected
  ✗ Windsurf     not detected

  Restart your editor for changes to take effect.
  Test with: cgx mcp (should print JSON on stdin)
```

Add `--dry-run` flag that prints what would be written without writing anything.

---

### Part B — Skills System

The Skills system requires zero configuration from the user. It works in every
AI coding assistant that can read markdown files and execute terminal commands.
It is generated automatically at the end of every `cgx analyze` run.

#### 8.4 — Skill Template (`crates/cgx-core/src/skill.rs`)

Define a `SKILL_TEMPLATE` constant (a raw string literal) that is the full
content of `CGX_SKILL.md`. Use `{{ placeholder }}` tokens for dynamic content.

The template must follow this exact structure:

```markdown
# cgx — Codebase Knowledge Graph

> Auto-generated by cgx on {{ indexed_at }}. Do not edit manually.
> Re-run `cgx analyze` to refresh.

## When to Use cgx

Use cgx commands INSTEAD of reading source files when you need to:
- Find where any function, class, or variable is defined
- Understand what depends on a specific piece of code
- Know what will break before making a change
- Understand the architecture of an unfamiliar area
- Find who owns a file or module
- Find dead code or unused exports
- Trace a full call chain from entry point to implementation

**Rule: Never open a file speculatively. Query first. Open only if you need
the implementation body — not to find where something lives.**

## Trigger Patterns

Run cgx automatically when the user says or implies any of:
- "what calls X" / "who uses X" / "what depends on X"
- "show me the architecture" / "how does this work"
- "what breaks if I change X" / "blast radius of X"
- "find X in the codebase" / "where is X defined"
- "who owns X" / "who wrote X"
- "is X used anywhere" / "is X dead code"
- Starting a new task in an unfamiliar part of the codebase
- Before making any edit to a function with many callers

## Commands

```bash
# Always run first in a new session
cgx summary

# Find any symbol
cgx query find <name>
cgx query find <name> --kind=Function

# Dependencies of a node
cgx query deps <node-name>

# Blast radius — run BEFORE every edit
cgx query blast-radius <function-name>

# Trace a call path
cgx query chain "<A> -> <B>"

# High-risk files
cgx hotspots

# Code ownership
cgx query owners <path>

# Search by concept
cgx query search "<phrase>"

# Community / cluster
cgx query community <id-or-name>

# Dead code
cgx query dead-code
```

## Workflow: Starting a Task

1. `cgx summary`                        — orient yourself
2. `cgx query find <entry-point>`       — locate the relevant node
3. `cgx query blast-radius <node>`      — know the risk before touching it
4. Open only the specific files you need

## Workflow: Before Every Edit

1. `cgx query blast-radius <function>`  — what breaks?
2. `cgx query deps <function>`          — what does it depend on?
3. Make the change
4. `cgx query blast-radius <function>`  — verify ripple is as expected

## Token Budget

| Action                    | Approx tokens |
|---------------------------|---------------|
| `cgx summary`             | ~400          |
| `cgx query find X`        | ~200          |
| `cgx query blast-radius X`| ~300-800      |
| Opening one source file   | ~2,000-15,000 |

Prefer 3 cgx queries over opening 1 file speculatively.

## This Codebase

- **Indexed:** {{ indexed_at }}
- **Nodes:** {{ node_count }} ({{ function_count }} functions,
  {{ class_count }} classes, {{ file_count }} files)
- **Edges:** {{ edge_count }}
- **Languages:** {{ language_breakdown }}
- **Communities:** {{ community_count }}

### Top Communities
{{ top_communities_list }}

### Hotspots (highest risk — review carefully before editing)
{{ hotspots_list }}

### Entry Points (nothing imports these — safe starting points)
{{ entry_points_list }}

### Most Depended-On Nodes (god nodes — change with extreme care)
{{ god_nodes_list }}
```

#### 8.5 — Skill Generator (`crates/cgx-core/src/skill.rs`)

```rust
pub struct SkillData {
    pub indexed_at: String,
    pub node_count: u64,
    pub function_count: u64,
    pub class_count: u64,
    pub file_count: u64,
    pub edge_count: u64,
    pub language_breakdown: String,   // "TypeScript 68%, Python 32%"
    pub community_count: u32,
    pub top_communities: Vec<CommunityInfo>,
    pub hotspots: Vec<Node>,          // top 5
    pub entry_points: Vec<Node>,      // top 5 (0 in-degree)
    pub god_nodes: Vec<Node>,         // top 5 (highest in-degree)
}

pub fn generate_skill(data: &SkillData) -> String {
    // Replace all {{ placeholders }} in SKILL_TEMPLATE with data
    // Format top_communities as a markdown list
    // Format hotspots as: "- `src/auth.ts` — churn 0.91, 14 callers"
    // Format entry_points as: "- `src/main.ts`"
    // Format god_nodes as: "- `db.query` — 47 callers"
}

pub fn write_skill(repo_root: &Path, data: &SkillData) -> Result<()> {
    let content = generate_skill(data);
    let path = repo_root.join("CGX_SKILL.md");
    std::fs::write(&path, content)?;
    Ok(())
}
```

#### 8.6 — Auto-generate AGENTS.md

When `cgx analyze` completes, also write `AGENTS.md` to the repo root.
This is separate from `CGX_SKILL.md` — AGENTS.md is for human readers and
AI agents that need a prose summary. CGX_SKILL.md is the operational command
reference.

```markdown
# Codebase Architecture

> Auto-generated by cgx {{ indexed_at }}

## Overview
{{ node_count }} nodes across {{ file_count }} files.
Primary languages: {{ language_breakdown }}.
{{ community_count }} architectural communities detected.

## Module Map
{{ community_descriptions }}

## Hotspots
These files change frequently and have many dependents.
Review carefully before editing.
{{ hotspots_table }}

## Entry Points
These files have no inbound imports — they are roots.
{{ entry_points_list }}

## AI Integration
This repo is indexed by cgx. Two integration modes are available:

**Skills (zero config):** Read `CGX_SKILL.md` for command reference.

**MCP (structured):** Run `cgx setup` to configure your editor,
then `cgx mcp` to start the server.
```

#### 8.7 — Git Hooks Installer (`crates/cgx-cli/src/cmd/analyze.rs`)

At the end of `cgx analyze`, install two git hooks if `.git/` exists:

```bash
# .git/hooks/post-commit
#!/bin/sh
cgx analyze --incremental --quiet
```

```bash
# .git/hooks/post-checkout
#!/bin/sh
cgx analyze --incremental --quiet
```

Rules:
- If the hook file already exists and was NOT written by cgx, print a warning
  and do NOT overwrite it. Instead print: "Skipped post-commit hook — file
  exists. Add `cgx analyze --incremental --quiet` manually."
- Detect cgx-written hooks by checking for a `# cgx-managed` comment on line 2
- Always write `# cgx-managed` as line 2 of hooks cgx creates
- Make hook files executable (`chmod +x` via `std::fs::Permissions`)
- `--incremental` flag: only re-parse files changed since last index
- `--quiet` flag: suppress all output except errors

#### 8.8 — Integrate Everything into `cgx analyze`

The final step of `cgx analyze` (after clustering in Phase 4) must:

```
1. Compute SkillData from DuckDB stats
2. Write CGX_SKILL.md to repo root       [Phase 8.5]
3. Write AGENTS.md to repo root          [Phase 8.6]
4. Install git hooks if .git/ exists     [Phase 8.7]
5. Print completion summary:

   ✓ Graph indexed — 1,203 nodes, 3,847 edges

   Generated files:
     CGX_SKILL.md   — skill for any AI assistant (commit this)
     AGENTS.md      — architecture summary (commit this)

   AI editor integration:
     MCP server:  cgx setup  (Cursor, Claude Code, Windsurf)
     Skills:      CGX_SKILL.md is ready — works without any setup

   Explore:
     cgx view        terminal graph
     cgx view --web  browser graph
     cgx hotspots    high-risk files
```

### Done When
- `cgx mcp` starts, responds to `initialize`, lists 10 tools
- All 10 MCP tools return correct data from the ts-sample fixture
- `run_query` with DELETE is blocked with a clear error
- `cgx setup --dry-run` detects editors and prints what it would write
- `cgx analyze` produces `CGX_SKILL.md` with non-empty hotspots/community sections
- `cgx analyze` produces `AGENTS.md` with accurate node/edge counts
- Git hooks are installed and contain `# cgx-managed` on line 2
- Re-running `cgx analyze` does not overwrite non-cgx hooks
- `CGX_SKILL.md` re-generated with fresh stats on every analyze run

---

## Phase 9 — GitHub Pages Publisher

**Goal:** `cgx publish` pushes a self-contained interactive graph to GitHub Pages.

### Tasks

#### 9.1 — Build pipeline (`crates/cgx-cli/src/cmd/publish.rs`)

1. Run `npm run build --workspace=packages/web-ui` via `std::process::Command`
2. Read `cgx-graph.json` (or generate it with `export_json`)
3. Inject graph data into `dist/index.html` as an inline `<script>`:
   `window.__CGX_GRAPH__ = <json>;`
4. The built web UI checks `window.__CGX_GRAPH__` before fetching from the API

#### 9.2 — Git push to `gh-pages` branch

Using `git2`:
1. Open the current repo
2. Get or create `origin` remote
3. Read all files from `dist/` into a tree
4. Create a commit on `refs/remotes/origin/gh-pages` (or create the branch)
5. Force push to `origin gh-pages`

Handle auth: try SSH key first (`~/.ssh/id_rsa`, `~/.ssh/id_ed25519`),
fall back to HTTP credential helper.
If auth fails, print clear instructions for setting up a GitHub token.

#### 9.3 — Output

After successful push, print:
```
  ✓ Graph published to GitHub Pages

  Live URL:   https://owner.github.io/repo-name/
  Embed code: <iframe src="https://owner.github.io/repo-name/" width="800" height="600"></iframe>
  Badge:      [![cgx graph](https://img.shields.io/badge/cgx-graph-blue)](https://owner.github.io/repo-name/)
```

### Done When
- `cgx publish` on a GitHub repo creates a working GitHub Pages site
- The published site renders the graph without any server calls
- Auth failure prints a clear, actionable error message

---

## Phase 10 — Graph Diff + Impact Analysis

**Goal:** `cgx diff` and `cgx impact` — understand how the graph changed across commits.

### Tasks

#### 10.1 — Graph snapshot at commit (`crates/cgx-core/src/git.rs`)

Add `analyze_at_commit(repo: &Repository, commit_oid: Oid) -> Result<GraphSnapshot>`:
- Checkout the file tree at that commit into a temp dir (in-memory, using git2 tree APIs)
- Run the parse + resolve pipeline on that snapshot
- Return a `GraphSnapshot { nodes: Vec<NodeDef>, edges: Vec<EdgeDef>, commit: String }`

#### 10.2 — Graph diff (`crates/cgx-core/src/diff.rs`)

```rust
pub struct GraphDiff {
    pub added_nodes: Vec<NodeDef>,
    pub removed_nodes: Vec<NodeDef>,
    pub added_edges: Vec<EdgeDef>,
    pub removed_edges: Vec<EdgeDef>,
    pub modified_nodes: Vec<(NodeDef, NodeDef)>,  // (before, after)
}

pub fn diff_graphs(before: &GraphSnapshot, after: &GraphSnapshot) -> GraphDiff;
```

#### 10.3 — `cgx diff <commit>` command

Show graph diff between HEAD and specified commit:
```
  GRAPH DIFF: HEAD vs HEAD~5
  ─────────────────────────────────────────────
  + Added:    12 nodes, 31 edges
  - Removed:   3 nodes,  8 edges
  ~ Modified:  7 nodes (churn scores changed)

  NEW NODES:
  + fn:src/payments/stripe.ts:createIntent  (Function)
  + cls:src/payments/PaymentService.ts      (Class)

  REMOVED NODES:
  - fn:src/legacy/pay.ts:processCard        (Function)

  NEW EDGES:
  + PaymentController → PaymentService (CALLS)
  + PaymentService → StripeClient (IMPORTS)
```

#### 10.4 — `cgx impact --since=7d` command

Find all nodes whose source files were modified in the last N days.
Then walk the graph forward (all things that CALL or IMPORT those nodes).
Print a ripple report:
```
  IMPACT ANALYSIS — last 7 days
  ─────────────────────────────────────────────
  Changed:   src/auth/service.ts (3 commits)
  Ripple:    → AuthController.login
             → Router.handleAuth
             → Middleware.validateToken
             → (3 test files)

  Changed:   src/db/pool.ts (1 commit)
  Ripple:    → 14 files depend on this (HIGH RISK)
```

### Done When
- `cgx diff HEAD~5` produces a correct, readable diff on a real repo
- `cgx impact --since=7d` shows accurate downstream nodes
- Both commands handle edge cases: no changes found, detached HEAD, etc.

---

## Final Integration Checklist

Run this before declaring the project complete:

```bash
# 1. Scaffold works
cargo build --workspace && npm run build

# 2. Analysis pipeline works on itself (dogfood)
cgx analyze .

# 3. All visualizations work
cgx view
cgx view --web

# 4. All exports produce valid output
cgx export --format=json | python3 -c "import sys,json; json.load(sys.stdin); print('valid')"
cgx export --format=mermaid
cgx export --format=dot
cgx export --format=graphml

# 5. Git intelligence works
cgx hotspots
cgx blame-graph

# 6. MCP starts and responds
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | cgx mcp

# 7. All tests pass
cargo test --workspace

# 8. No panics or unwrap() in library code
cargo clippy --workspace -- -D clippy::unwrap_used -D clippy::panic
```

---

## Prioritization If You Must Cut Scope

Must have (ship without these = no point):
- Phase 0, 1, 2 — parsing + storage
- Phase 5 (JSON) — export
- Phase 7 — web UI (this is the main value prop)

Should have:
- Phase 3 — git layer (differentiator)
- Phase 6 — TUI
- Phase 8 — MCP

Nice to have:
- Phase 4 — Leiden clustering
- Phase 9 — GitHub Pages publisher
- Phase 10 — Graph diff

Start with Phases 0→1→2→5→7 for a working MVP that already beats both GitNexus and Graphify on UX.
