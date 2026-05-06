# cgx-engine

Core engine for [cgx](https://github.com/AayushBahukhandi/cgx) — the library that turns any Git repository into a queryable knowledge graph.

## What it does

This crate contains all the heavy lifting:

- **Tree-sitter parsing** — Extracts functions, classes, imports, and calls from TypeScript, JavaScript, Python, Rust, Go, Java, and PHP
- **DuckDB graph storage** — Stores nodes and edges in an embedded database (~/.cgx/repos/)
- **Git analysis** — Computes churn scores, co-change edges, and ownership from git history
- **Community detection** — Uses the Leiden algorithm to auto-cluster your codebase into modules
- **Export formats** — JSON, Mermaid, DOT, SVG, GraphML
- **Skill generation** — Auto-generates CGX_SKILL.md and AGENTS.md for AI assistants

## Architecture

```
walker.rs      → Discovers source files by language
parser.rs      → Trait for language parsers
parsers/       → One parser per language (ts, py, rs, go, java, php)
graph.rs       → DuckDB schema, queries, upserts
git.rs         → Churn, co-change, blame analysis
cluster.rs     → Leiden community detection
export.rs      → Multi-format graph export
skill.rs       → CGX_SKILL.md / AGENTS.md generation
config.rs      → .cgx/config.toml schema
diff.rs        → Graph diff between commits
resolver.rs    → Cross-file import resolution
registry.rs    → Indexed repo registry
```

## Usage

This is a library crate. Most users will use the CLI (`cgx-cli`) instead. But you can use it directly:

```rust
use cgx_engine::{walk_repo, GraphDb, analyze_repo};

let files = walk_repo(std::path::Path::new("./my-project"))?;
let db = GraphDb::open(std::path::Path::new("./my-project"))?;
```

## License

MIT
