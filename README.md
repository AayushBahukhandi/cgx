<div align="center">

<!-- TODO: Replace docs/demo.gif with your actual demo GIF -->
<img src="docs/demo.gif" alt="cgx demo" width="100%" />

<br />

# cgx

**Turn any Git repository into a queryable knowledge graph.**

[![CI](https://github.com/AayushBahukhandi/cgx/actions/workflows/ci.yml/badge.svg)](https://github.com/AayushBahukhandi/cgx/actions)
[![crates.io](https://img.shields.io/crates/v/cgx-cli.svg)](https://crates.io/crates/cgx-cli)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Graph](https://img.shields.io/badge/cgx-live%20graph-blue)](https://aayushbahukhandi.github.io/cgx/)

[**Live Demo**](https://aayushbahukhandi.github.io/cgx/) · [**Documentation**](docs/) · [**Releases**](https://github.com/AayushBahukhandi/cgx/releases)

</div>

---

> **A codebase has two graphs.**
> The structural graph — what calls what.
> The temporal graph — what changes with what.
> No existing tool shows you both. cgx does.

---

## What cgx Solves

You have a codebase. You need to understand it, refactor it, or explain it to an AI assistant. The normal approach is reading files — slow, expensive in tokens, and impossible to do at scale.

cgx indexes your entire repo once — parsing every function, class, and import with Tree-sitter, then overlaying your full Git history to build a co-change graph. The result is a queryable knowledge graph stored locally in DuckDB that answers architectural questions in milliseconds.

```bash
# Instead of reading 40 files to understand blast radius:
cgx query blast-radius "AuthService"
# → 14 direct callers, 67 total affected. Risk: HIGH. Done in 0.3s.

# Instead of grep-ing for ownership:
cgx hotspots
# → Top 5 files ranked by churn × coupling. The ones to worry about.

# Instead of asking your AI to read everything:
# CGX_SKILL.md is auto-generated in your repo root.
# Your AI assistant reads it and queries the graph instead of files.
# 71x fewer tokens. Same answers.
```

---

## Features

| Feature | Description |
|---|---|
| **AST Parsing** | Tree-sitter parses TS/TSX, JS/JSX, Python, Rust, Go, Java, PHP in parallel |
| **JSX Caller Tracking** | React component usages (`<MyComp />`) are tracked as call edges |
| **Git Intelligence** | Churn scores, co-change edges, ownership — the temporal graph |
| **DuckDB Storage** | Zero-server embedded graph database. Instant queries. |
| **Community Detection** | Leiden algorithm auto-clusters your codebase into modules |
| **Terminal TUI** | Force-directed graph in Ratatui. Works over SSH. |
| **WebGL Browser Graph** | Sigma.js renders thousands of nodes at 60fps |
| **AI Chat** | Ask questions about your code in natural language. Ollama supported. |
| **MCP Server** | 10 typed tools for Cursor, Claude Code, Windsurf |
| **Skills System** | `CGX_SKILL.md` auto-generated — works in any AI assistant |
| **Share Links** | `cgx share` uploads your graph to a Gist — anyone views it in a browser, no install needed |
| **GitHub Pages Publish** | `cgx publish` pushes a self-contained graph site to your `gh-pages` branch |
| **Graph Diff** | See how your architecture changed between commits |
| **Dead Code Detection** | Find unreferenced exports across the whole codebase |
| **Self-contained binary** | Web UI is embedded in the binary — Homebrew and `cargo install` work out of the box |

---

## Installation

### cargo

```bash
cargo install cgx-cli
```

The installed binary is named `cgx`. If `cgx --version` prints `command not found`, add Cargo's bin directory to your PATH:

```bash
# zsh (~/.zshrc) or bash (~/.bashrc / ~/.bash_profile)
export PATH="$HOME/.cargo/bin:$PATH"
```

```fish
# fish (~/.config/fish/config.fish)
fish_add_path "$HOME/.cargo/bin"
```

> `cargo install` compiles from source and may take a few minutes. Run it again to update.

### Pre-built binary (Windows, macOS, Linux)

Download from [GitHub Releases](https://github.com/AayushBahukhandi/cgx/releases/latest).

```bash
# macOS arm64 (Apple Silicon)
curl -L https://github.com/AayushBahukhandi/cgx/releases/latest/download/cgx-v0.1.3-aarch64-apple-darwin.tar.gz | tar xz
sudo mv cgx /usr/local/bin/

# macOS x86_64 (Intel)
curl -L https://github.com/AayushBahukhandi/cgx/releases/latest/download/cgx-v0.1.3-x86_64-apple-darwin.tar.gz | tar xz
sudo mv cgx /usr/local/bin/

# Linux x86_64
curl -L https://github.com/AayushBahukhandi/cgx/releases/latest/download/cgx-v0.1.3-x86_64-unknown-linux-gnu.tar.gz | tar xz
sudo mv cgx /usr/local/bin/
```

Windows users: download the `.zip` from the releases page and place `cgx.exe` in a directory on your `%PATH%`.

### Homebrew (macOS / Linux)

> **Coming soon.** A tap is planned. Until then use `cargo install cgx-cli` or the pre-built binary above.

### Verify

```bash
cgx --version   # should print 0.1.3
cgx doctor      # checks your setup and editor integrations
```

---

## Quick Start

```bash
# 1. Index your repo
cd your-project
cgx analyze

# 2. Open the browser graph (auto-analyzes if not indexed yet)
cgx view --web

# 3. Share with anyone — no install required on their end
cgx share

# 4. See your riskiest files
cgx hotspots

# 5. Set up your AI editor
cgx setup
```

After `cgx analyze`, two files appear in your repo root:
- `CGX_SKILL.md` — tells your AI assistant how to query the graph instead of reading files
- `AGENTS.md` — a prose architecture summary: communities, hotspots, entry points, god nodes

Both regenerate automatically on every `git commit` via installed hooks.

---

## Core Commands

### Analysis

```bash
cgx analyze                    # index current repo
cgx analyze ./path             # index any local path
cgx analyze --watch            # live-reload on file save
cgx analyze --incremental      # re-parse only changed files (used by git hooks)
cgx analyze --no-git           # skip git history layer
cgx analyze --force            # full clean re-index
```

### Visualize

```bash
cgx view                       # terminal TUI (works over SSH)
cgx view --web                 # browser WebGL graph — auto-analyzes if not indexed
cgx view --community=3         # scope TUI view to a cluster
```

> In the terminal TUI, press `e` on a selected node to view its ego-graph (neighbors up to 2 hops).

### Share

```bash
cgx share                      # upload graph to a GitHub Gist → hosted viewer URL
cgx share --token ghp_xxx      # use a specific GitHub token
cgx share --public             # make the Gist public (default: secret)
```

`cgx share` requires a GitHub token with `gist` scope. It uses (in order): `--token`, `GITHUB_TOKEN` env var, or `gh auth token` if you have the GitHub CLI installed.

The returned URL looks like:
```
https://aayushbahukhandi.github.io/cgx/?data=https://gist.githubusercontent.com/...
```
Anyone can open that link in a browser — no cgx install needed.

### Publish to GitHub Pages

```bash
cgx publish                    # push self-contained graph site to gh-pages branch
cgx publish --dry-run          # preview what would be pushed
cgx publish --badge            # print README badge markdown
```

### Query

```bash
cgx query find "AuthService"            # locate any symbol
cgx query find "login" --kind=Function  # filter by kind
cgx query blast-radius "deleteUser"     # what breaks if this changes?
cgx query chain "Router.handleLogin"    # trace a call chain
cgx query dead-code                     # unreferenced exports
cgx query search "session management"  # full-text search
cgx query owners src/payments/          # git blame ownership
```

### Git Intelligence

```bash
cgx hotspots                   # high churn × high coupling = danger zone
cgx blame-graph                # ownership by contributor
cgx diff HEAD~5                # architecture diff between commits
```

### Export

```bash
cgx export --format=json       # full graph JSON
cgx export --format=mermaid    # paste into any README
cgx export --format=graphml    # import into Gephi / yEd
```

### Maintenance

```bash
cgx summary                    # repo stats: nodes, edges, languages, communities
cgx doctor                     # run diagnostics on your install
cgx clean                      # remove indexed data for current repo
cgx clean --all                # remove ALL indexed repos
cgx status                     # show index status for registered repos
```

---

## AI Integration

### Method 1 — Skills (works everywhere, zero config)

After `cgx analyze`, a `CGX_SKILL.md` file appears in your repo root.
Any AI assistant that can read files and run terminal commands — Claude Code,
Cursor, GitHub Copilot Chat, Gemini CLI — will automatically use it.

The skill file tells your AI:
- When to call `cgx query` instead of reading source files
- The exact command for every type of question
- Live stats about your codebase baked in (hotspots, communities, entry points)

**Result:** Your AI stops reading 40 files to answer an architectural question
and runs one `cgx query` command instead. 71x fewer tokens. Same answer.

### Method 2 — MCP Server (for Cursor, Claude Code, Windsurf)

```bash
cgx setup    # auto-detects your editors and writes their MCP configs
```

Restart your editor. cgx now exposes 10 typed tools your AI can call directly:

| Tool | What it answers |
|---|---|
| `get_repo_summary` | Full architectural overview — called first every session |
| `find_symbol` | Where is X defined? File + line |
| `get_neighbors` | What does X depend on? What depends on X? |
| `get_blast_radius` | What breaks if I change X? Risk level. |
| `get_call_chain` | Trace from A to B through the call graph |
| `get_community` | All nodes in the auth/db/payments cluster |
| `search_graph` | Full-text search over all symbol names |
| `get_hotspots` | Highest churn × coupling nodes |
| `get_file_owners` | Git blame ownership for any file |
| `run_query` | Raw SQL SELECT against the graph (read-only) |

**Example:** Ask "refactor the login function to add rate limiting" in Claude Code.
It calls `get_blast_radius`, `get_neighbors`, and `get_file_owners` — 3 tool calls,
under 3,000 tokens — then writes the code knowing exactly what it needs to update.

---

## AI Chat

The browser UI (`cgx view --web`) includes a built-in chat panel.
Ask natural language questions about your codebase.

### Supported AI Providers

#### OpenAI
```bash
export OPENAI_API_KEY=sk-...
cgx serve
```

#### Anthropic
```bash
export ANTHROPIC_API_KEY=sk-ant-...
export CGX_CHAT_PROVIDER=anthropic
export CGX_CHAT_MODEL=claude-haiku-4-5
cgx serve
```

#### Ollama (fully local, no API key)
```bash
ollama pull codellama
export CGX_CHAT_PROVIDER=ollama
export CGX_CHAT_MODEL=codellama
cgx serve
```

#### Any OpenAI-compatible API
```bash
export CGX_CHAT_PROVIDER=openai-compatible
export CGX_CHAT_BASE_URL=https://api.together.ai/v1
export CGX_CHAT_API_KEY=your-key
export CGX_CHAT_MODEL=meta-llama/Llama-3-70b-chat-hf
cgx serve
```

> **Privacy note:** cgx chat sends only graph metadata to the AI — node names,
> file paths, churn scores, community labels. It never sends your source code.
> With Ollama, nothing leaves your machine.

---

## Git Intelligence — The Differentiator

Every other codebase analysis tool only knows the **structural graph** —
what imports what right now. cgx also builds the **temporal graph** from
your git history.

**Co-change edges** — files that always change together in commits,
even if they don't import each other. Hidden coupling.

**Churn scores** — how frequently each node changes, normalized 0–1.
Combined with in-degree, this gives you the hotspot score.

**Ownership** — who owns what, by git blame line count.

---

## How cgx Compares

|  | cgx | GitNexus | Graphify |
|---|---|---|---|
| Tree-sitter parsing | ✅ | ✅ | ✅ |
| JSX/TSX caller tracking | ✅ | ❌ | ❌ |
| Cross-file resolution | ✅ | ✅ | ❌ |
| Git history (churn/blame) | ✅ | ❌ | ❌ |
| Co-change graph | ✅ | ❌ | ❌ |
| Dead code detection | ✅ | ❌ | ❌ |
| Terminal TUI | ✅ | ❌ | ❌ |
| WebGL browser graph | ✅ | ❌ | ✅ |
| AI Chat (multi-provider) | ✅ | ❌ | ❌ |
| Ollama / local LLM | ✅ | ❌ | ❌ |
| MCP server | ✅ | ✅ | ❌ |
| Skills system | ✅ | ❌ | ✅ |
| Share links (no install) | ✅ | ❌ | ❌ |
| GitHub Pages publish | ✅ | ❌ | ❌ |
| Self-contained binary | ✅ | ❌ | ❌ |
| LLM required for indexing | ❌ Never | ❌ Never | ✅ Always |
| License | MIT | Non-commercial | MIT |

---

## Supported Languages

| Language | Parser | Status |
|---|---|---|
| TypeScript / TSX | tree-sitter-typescript | ✅ Stable |
| JavaScript / JSX | tree-sitter-javascript | ✅ Stable |
| Python | tree-sitter-python | ✅ Stable |
| Rust | tree-sitter-rust | ✅ Stable |
| Go | tree-sitter-go | ✅ Stable |
| Java | tree-sitter-java | ✅ Stable |
| PHP | tree-sitter-php | ✅ Stable |
| C# | tree-sitter-java (fallback) | 🔧 Beta |
| C / C++ | — | 📋 Planned |
| Swift | — | 📋 Planned |
| Ruby | — | 📋 Planned |

Want a language added? [Open an issue](https://github.com/AayushBahukhandi/cgx/issues/new) or submit a PR — new parsers are one file in `crates/cgx-engine/src/parsers/`.

---

## Configuration

cgx has no config file for basic usage. Everything has sensible defaults.

For advanced configuration, create `.cgx/config.toml` in your repo root:

```toml
[analyze]
# Languages to parse (default: all supported)
languages = ["typescript", "javascript", "python"]

# Directories to skip beyond .gitignore
exclude = ["vendor/", "generated/", "*.pb.go"]

# Git history window for churn calculation (days)
churn_window_days = 90

# Minimum co-change count to create a CO_CHANGES edge
co_change_threshold = 2

[chat]
# Default provider (openai | anthropic | ollama | openai-compatible)
provider = "ollama"
model = "codellama"
ollama_host = "http://localhost:11434"

[serve]
port = 7373
auto_open = true

[skill]
auto_generate = true
```

---

## Architecture

cgx is built in Rust (core engine) and TypeScript (web UI).

```
cgx-engine  — Tree-sitter parsing, DuckDB storage, git analysis,
              Leiden clustering, export, skill generation
cgx-cli     — All user-facing commands, TUI (Ratatui), HTTP server (Axum),
              web UI embedded via rust-embed (self-contained binary)
cgx-mcp     — MCP stdio server (JSON-RPC 2.0)
web-ui      — Vite + React + Sigma.js WebGL graph
```

The graph is stored locally at `~/.cgx/repos/<hash>.db` — one DuckDB file
per repo. No external services, no cloud, no network required for local use.

---

## Contributing

cgx is MIT licensed and welcomes contributions.

```bash
git clone https://github.com/AayushBahukhandi/cgx
cd cgx
cargo build --workspace
npm install && npm run build

# Run tests
cargo test --workspace

# Lint
cargo clippy --workspace -- -D warnings -D clippy::unwrap_used
cargo fmt --all
```

**Best places to contribute:**
- New language parsers — one file in `crates/cgx-engine/src/parsers/`
- New export formats
- TUI improvements
- Web UI features

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full guide.

---

## Roadmap

- [ ] Homebrew formula + tap
- [ ] `cgx changelog` — generate changelogs from graph diffs
- [ ] VS Code extension
- [ ] `cgx watch` with debounced incremental indexing
- [ ] Mermaid diagram auto-commit to docs/ on every push (GitHub Action)
- [ ] Ruby, Swift, C/C++ parsers
- [ ] `cgx init` — guided first-run experience
- [ ] cgx cloud — shared graphs for teams (hosted)

---

## Setting Up Distribution

### crates.io

1. Set the `CARGO_REGISTRY_TOKEN` secret in your GitHub repo settings.
2. The release workflow (`.github/workflows/release.yml`) publishes all three crates automatically on every `v*` tag.
3. Users install with `cargo install cgx-cli`.

### GitHub Pages (hosted viewer)

Already live at `https://aayushbahukhandi.github.io/cgx/`. The `deploy-pages.yml` workflow rebuilds and redeploys it on every release tag automatically.

### Homebrew

A tap requires a separate `homebrew-cgx` repo. After the first release builds, get the SHA256 hashes:

```bash
shasum -a 256 cgx-v0.1.3-aarch64-apple-darwin.tar.gz
shasum -a 256 cgx-v0.1.3-x86_64-apple-darwin.tar.gz
shasum -a 256 cgx-v0.1.3-x86_64-unknown-linux-gnu.tar.gz
```

Then create `Formula/cgx.rb` in `homebrew-cgx` pointing at the release tarballs with those hashes.

---

## License

MIT — use it in anything, commercial or otherwise.

---

<div align="center">

Built with Rust 🦀 · Tree-sitter · DuckDB · Sigma.js

**[Star this repo](https://github.com/AayushBahukhandi/cgx) if cgx saves you time.**

</div>
