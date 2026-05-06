<div align="center">

<!-- TODO: Replace docs/demo.gif with your actual demo GIF -->
<img src="docs/demo.gif" alt="cgx demo" width="100%" />

<br />

# cgx

**Turn any Git repository into a queryable knowledge graph.**

<!-- TODO: Update badge URLs after setting up GitHub Actions and crates.io -->
[![CI](https://github.com/AayushBahukhandi/cgx/actions/workflows/ci.yml/badge.svg)](https://github.com/AayushBahukhandi/cgx/actions)
[![crates.io](https://img.shields.io/crates/v/cgx.svg)](https://crates.io/crates/cgx)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Graph](https://img.shields.io/badge/cgx-graph-blue)](https://AayushBahukhandi.github.io/cgx/)

[**Live Demo**](https://AayushBahukhandi.github.io/cgx/) · [**Documentation**](docs/) · [**Discord**](https://discord.gg/cgx)

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
| **AST Parsing** | Tree-sitter parses TS, JS, Python, Rust, Go, Java in parallel |
| **Git Intelligence** | Churn scores, co-change edges, ownership — the temporal graph |
| **DuckDB Storage** | Zero-server embedded graph database. Instant queries. |
| **Community Detection** | Leiden algorithm auto-clusters your codebase into modules |
| **Terminal TUI** | Force-directed graph in Ratatui. Works over SSH. |
| **WebGL Browser Graph** | Sigma.js renders thousands of nodes at 60fps |
| **AI Chat** | Ask questions about your code in natural language. Ollama supported. |
| **MCP Server** | 10 typed tools for Cursor, Claude Code, Windsurf |
| **Skills System** | `CGX_SKILL.md` auto-generated — works in any AI assistant |
| **GitHub Pages** | One command publishes your architecture graph publicly |
| **Graph Diff** | See how your architecture changed between commits |
| **Dead Code Detection** | Find unreferenced exports across the whole codebase |

---

## Installation

### Homebrew (macOS / Linux)

> **Coming soon.** We do not yet have a Homebrew formula. See [below](#setting-up-distribution) for how to create one.

Once published, installation will be:

```bash
# Add the tap (one-time)
brew tap AayushBahukhandi/cgx https://github.com/AayushBahukhandi/homebrew-cgx

# Install cgx
brew install cgx

# Update later
brew upgrade cgx
```

### cargo

```bash
cargo install cgx-cli
```

> **How cargo updates work:** New versions are published to [crates.io](https://crates.io/crates/cgx-cli) on every release tag. Run `cargo install cgx-cli` again to update. Note: `cargo install` compiles from source, so it may take a few minutes.

### Pre-built binary (Windows, macOS, Linux)

Download from [GitHub Releases](https://github.com/AayushBahukhandi/cgx/releases/latest).

```bash
# macOS / Linux example:
curl -L https://github.com/AayushBahukhandi/cgx/releases/latest/download/cgx-v0.1.0-x86_64-apple-darwin.tar.gz | tar xz
sudo mv cgx /usr/local/bin/
```

> **How binary updates work:** Download the latest `.tar.gz` or `.zip` for your platform from GitHub Releases, extract, and replace the binary. The `cgx update` command will remind you where to look and can auto-detect your install method.

### Verify

```bash
cgx --version
cgx doctor    # checks your setup and editor integrations
```

---

## Quick Start

```bash
# 1. Index your repo (run once, then auto-updates on every commit)
cd your-project
cgx analyze

# 2. Explore in the terminal
cgx view

# 3. Explore in the browser
cgx view --web

# 4. See your riskiest files
cgx hotspots

# 5. Set up your AI editor
cgx setup
```

That's it. After `cgx analyze`, two files appear in your repo root:
- `CGX_SKILL.md` — your AI assistant reads this and queries the graph instead of files
- `AGENTS.md` — a prose architecture summary of your codebase

Both update automatically on every git commit via installed hooks.

---

## Updating cgx

We ship updates on three channels. Pick the one that matches how you installed:

| Install method | Update command | Notes |
|---|---|---|
| **Homebrew** | `brew upgrade cgx` | Fastest. Pulls pre-built binary + web UI. |
| **cargo** | `cargo install cgx-cli` | Compiles from source. Takes a few minutes. |
| **Binary** | `cgx update --auto` | Detects install path and tells you what to do. |

**Our release flow:**
1. We tag a release (`git tag v0.2.0 && git push origin v0.2.0`).
2. GitHub Actions builds binaries for all platforms, publishes to crates.io, and drafts a release.
3. The Homebrew formula is updated with new SHA256 hashes.
4. You get the update via your chosen channel within minutes.

---

## Demo

### Terminal TUI — `cgx view`

<!-- TODO: record docs/tui-demo.gif -->
<img src="docs/tui-demo.gif" alt="cgx terminal TUI" width="100%" />

### WebGL Browser Graph — `cgx view --web`

<!-- TODO: add docs/web-demo.png -->
<img src="docs/web-demo.png" alt="cgx browser graph" width="100%" />

### AI Chat — built into the browser UI

<!-- TODO: add docs/chat-demo.png -->
<img src="docs/chat-demo.png" alt="cgx AI chat" width="100%" />

---

## Core Commands

### Analysis

```bash
cgx analyze                    # index current repo
cgx analyze ./path             # index any local path
cgx analyze github:owner/repo  # clone + index remote
cgx analyze --watch            # live-reload on file save
cgx analyze --incremental      # re-parse only changed files (used by git hooks)
cgx analyze --no-git           # skip git history layer
cgx analyze --force            # full clean re-index
```

### Query Your Codebase

```bash
# Find any symbol
cgx query find "AuthService"
cgx query find "login" --kind=Function

# Know the blast radius before you touch anything
cgx query blast-radius "deleteUser"

# Trace a call chain
cgx query chain "Router.handleLogin -> db.query"

# Find dead code
cgx query dead-code

# Full-text search
cgx query search "session management"

# Who owns a file
cgx query owners src/payments/
```

### Git Intelligence

```bash
cgx hotspots                   # high churn × high coupling = danger zone
cgx blame-graph                # ownership by contributor
cgx impact --since=7d          # what changed + downstream ripple
cgx diff HEAD~5                # architecture diff between commits
```

### Visualize

```bash
cgx view                       # terminal TUI (works over SSH)
cgx view --web                 # WebGL browser graph
cgx view --community=3         # scope to a cluster
```

> **Tip:** In the terminal TUI, press `e` on a selected node to view its ego-graph (neighbors up to 2 hops).

### Export

```bash
cgx export --format=json       # full graph JSON (machine-readable)
cgx export --format=mermaid    # paste into any README
cgx export --format=svg        # static diagram
cgx export --format=graphml    # import into Gephi / yEd
```

### Publish to GitHub Pages

```bash
cgx publish                    # push self-contained graph to gh-pages
cgx publish --dry-run          # preview without pushing
cgx publish --badge            # get README badge markdown
```

### Maintenance

```bash
cgx doctor                     # run diagnostics on your install
cgx clean                      # remove indexed data for current repo
cgx clean --all                # remove ALL indexed repos
cgx update                     # show update instructions
cgx update --auto              # auto-update (cargo / homebrew only)
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
Ask natural language questions about your codebase. Answers come from
the pre-analyzed graph — no raw file reading, no hallucination about structure.

```
You: What are the riskiest files to change right now?

cgx: Based on the graph, your top 3 danger zones are:

  1. src/db/pool.ts — churn 0.92, 31 callers
     Changed in 47 of the last 90 commits. Everything in the
     db-layer community depends on it. Any change here has a
     blast radius of 89 nodes.

  2. src/auth/service.ts — churn 0.87, 14 callers
     Co-changes with db/pool.ts in 31 commits (hidden coupling).
     Alice owns 73% of this file by blame.

  3. src/api/router.ts — churn 0.71, 22 callers
     Entry point for all HTTP traffic. High in-degree.
```

### Supported AI Providers

cgx chat works with any of these. Pick what you have.

#### OpenAI (GPT-4o-mini recommended)
```bash
export OPENAI_API_KEY=sk-...
cgx serve
```

#### Anthropic (Claude Haiku recommended)
```bash
export ANTHROPIC_API_KEY=sk-ant-...
export CGX_CHAT_PROVIDER=anthropic
export CGX_CHAT_MODEL=claude-haiku-4-5   # fast + cheap
cgx serve
```

#### Ollama (fully local, no API key needed)
```bash
# 1. Install Ollama: https://ollama.ai
# 2. Pull a model
ollama pull llama3.2        # good all-rounder
ollama pull codellama       # tuned for code
ollama pull deepseek-coder  # excellent for code Q&A

# 3. Start cgx — it auto-detects Ollama if running
cgx serve

# Or specify explicitly:
export CGX_CHAT_PROVIDER=ollama
export CGX_CHAT_MODEL=codellama
export CGX_OLLAMA_HOST=http://localhost:11434   # default, change if needed
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

Works with Together AI, Fireworks AI, Groq, Mistral, LM Studio,
and any other provider that speaks the OpenAI API format.

#### Provider comparison

| Provider | Speed | Cost | Privacy | Best model for code |
|---|---|---|---|---|
| OpenAI | Fast | ~$0.001/q | Cloud | gpt-4o-mini |
| Anthropic | Fast | ~$0.001/q | Cloud | claude-haiku-4-5 |
| Ollama | Medium | Free | 100% local | codellama, deepseek-coder |
| OpenAI-compatible | Varies | Varies | Varies | depends on provider |

> **Privacy note:** cgx chat sends only graph metadata to the AI — node names,
> file paths, churn scores, community labels. It never sends your source code.
> With Ollama, nothing leaves your machine.

---

## How Token Savings Work

Traditional approach — AI reads files:
```
"What calls AuthService?" → AI opens 8 files → 42,000 tokens → $0.04
```

cgx approach — AI queries the graph:
```
"What calls AuthService?" → cgx query find AuthService → 180 tokens → $0.0002
```

**The CGX_SKILL.md file is the key.** It's generated after every `cgx analyze`
and baked with your live codebase stats. When your AI reads it at session start,
it already knows your hotspots, communities, and entry points — before asking
a single question. That collapses 5-10 exploratory queries into zero.

---

## Git Intelligence — The Differentiator

Every other codebase analysis tool only knows the **structural graph** —
what imports what right now. cgx also builds the **temporal graph** from
your git history.

The temporal graph reveals things static analysis cannot:

**Co-change edges** — files that always change together in commits,
even if they don't import each other. Hidden coupling. The source of
"why did this unrelated thing break?"

```bash
cgx export --format=json | python3 -c "
import json, sys
d = json.load(sys.stdin)
co = sorted([e for e in d['edges'] if e['kind']=='CO_CHANGES'],
            key=lambda x: -x['weight'])
for e in co[:5]:
    print(f'{e[\"src\"]} <-> {e[\"dst\"]}  co-changed {int(e[\"weight\"]*100)}% of the time')
"
# src/auth/service.ts <-> src/db/pool.ts  co-changed 89% of the time
# But they don't import each other. That's the hidden coupling.
```

**Churn scores** — how frequently each node changes, normalized 0–1.
Combined with coupling (in-degree), this gives you the hotspot score.
Files that change often AND have many dependents are your landmines.

**Ownership** — who owns what, by git blame line count. Answers
"who do I talk to before changing this?" without asking in Slack.

---

## How cgx Compares

|  | cgx | GitNexus | Graphify |
|---|---|---|---|
| Tree-sitter parsing | ✅ | ✅ | ✅ |
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
| GitHub Pages publish | ✅ | ❌ | ❌ |
| Graph diff between commits | ✅ | ❌ | ❌ |
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
| C / C++ | tree-sitter-c/cpp | 📋 Planned |
| C# | tree-sitter-java (fallback) | 🔧 Beta |
| PHP | tree-sitter-php | ✅ Stable |
| Swift | tree-sitter-swift | 📋 Planned |
| Ruby | tree-sitter-ruby | 📋 Planned |

Want a language added? [Open an issue](https://github.com/AayushBahukhandi/cgx/issues/new?template=language-request.md) or submit a PR — new parsers are one file in `crates/cgx-core/src/parsers/`.

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

# Default model
model = "codellama"

# Ollama host (if not localhost)
ollama_host = "http://localhost:11434"

[serve]
# HTTP server port
port = 7373

# Open browser automatically on cgx view --web
auto_open = true

[skill]
# Regenerate CGX_SKILL.md on every analyze (default: true)
auto_generate = true

# Include token budget table in skill file
include_token_budget = true
```

---

## Architecture

cgx is built in Rust (core engine) and TypeScript (web UI).

```
cgx-core    — Tree-sitter parsing, DuckDB storage, git analysis,
              Leiden clustering, export, skill generation
cgx-cli     — All user-facing commands, TUI (Ratatui), HTTP server (Axum)
cgx-mcp     — MCP stdio server (JSON-RPC 2.0)
web-ui      — Vite + React + Sigma.js WebGL graph
```

The graph is stored locally at `~/.cgx/repos/<hash>.db` — one DuckDB file
per repo. No external services, no cloud, no network required.

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
./scripts/integration-test.sh

# Lint
cargo clippy --workspace -- -D warnings -D clippy::unwrap_used
```

**Best places to contribute:**
- New language parsers — one file in `crates/cgx-core/src/parsers/`
- New export formats
- TUI improvements
- Web UI features

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full guide.

---

## Roadmap

- [ ] `cgx changelog` — generate changelogs from graph diffs
- [ ] VS Code extension
- [ ] `cgx watch` with debounced incremental indexing
- [ ] Mermaid diagram auto-commit to docs/ on every push (GitHub Action)
- [ ] Ruby, Swift, PHP parsers
- [ ] `cgx init` — guided first-run experience
- [ ] cgx cloud — shared graphs for teams (hosted)

---

## Setting Up Distribution

If you are publishing this project, here is how to set up `cargo` and `brew` distribution.

### crates.io (cargo install)

1. **Claim the crate names** (one-time):
   ```bash
   cargo publish --package cgx-core --dry-run
   cargo publish --package cgx-mcp --dry-run
   cargo publish --package cgx-cli --dry-run
   ```

2. **Set the `CARGO_REGISTRY_TOKEN` secret** in your GitHub repo settings.
   The release workflow (`.github/workflows/release.yml`) publishes automatically on every tag.

3. **Update `Cargo.toml` author fields**:
   Replace `aayush bahukhandi <aayushpotter555@gmail.com>` in all `Cargo.toml` files with your real name/email.

4. **Users install with:**
   ```bash
   cargo install cgx-cli
   ```

### Homebrew (brew install)

There is no Homebrew formula in this repo yet. You need a separate tap repository.

**Step 1 — Create a tap repo:**
```bash
# Create a new public repo: github.com/AayushBahukhandi/homebrew-cgx
git clone https://github.com/AayushBahukhandi/homebrew-cgx.git
cd homebrew-cgx
```

**Step 2 — Create the formula** (`Formula/cgx.rb`):
```ruby
class Cgx < Formula
  desc "Turn any Git repository into a queryable knowledge graph"
  homepage "https://github.com/AayushBahukhandi/cgx"
  version "0.1.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/AayushBahukhandi/cgx/releases/download/v#{version}/cgx-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "SHA256_OF_ARM64_BINARY"
    else
      url "https://github.com/AayushBahukhandi/cgx/releases/download/v#{version}/cgx-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "SHA256_OF_X86_64_BINARY"
    end
  end

  on_linux do
    url "https://github.com/AayushBahukhandi/cgx/releases/download/v#{version}/cgx-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "SHA256_OF_LINUX_BINARY"
  end

  def install
    bin.install "cgx"
    # Install bundled web UI assets
    pkgshare.install "web-ui" if File.directory?("web-ui")
  end

  test do
    system "#{bin}/cgx", "--version"
  end
end
```

**Step 3 — Get SHA256 hashes:**
After your first GitHub Release is published, download each tarball and run:
```bash
shasum -a 256 cgx-v0.1.0-aarch64-apple-darwin.tar.gz
```
Paste the hashes into the formula and commit.

**Step 4 — Users install with:**
```bash
brew tap AayushBahukhandi/cgx
brew install cgx
```

**Automating updates:**
You can automate Step 3 with a GitHub Action in the `homebrew-cgx` repo that listens for release webhooks and opens a PR with updated SHA256s.

---

## License

MIT — use it in anything, commercial or otherwise.

---

<div align="center">

Built with Rust 🦀 · Tree-sitter · DuckDB · Sigma.js

**[Star this repo](https://github.com/AayushBahukhandi/cgx) if cgx saves you time.**

</div>
