<div align="center">

<a href="https://aayushbahukhandi.github.io/cgx/">
  <img src="https://raw.githubusercontent.com/AayushBahukhandi/cgx/main/assets/cgx-web-hero.gif" alt="cgx web graph demo" width="100%" />
</a>

<br /><br />

# cgx

**Turn any Git repository into a queryable knowledge graph.**

[![CI](https://github.com/AayushBahukhandi/cgx/actions/workflows/ci.yml/badge.svg)](https://github.com/AayushBahukhandi/cgx/actions)
[![crates.io](https://img.shields.io/crates/v/cgx-cli.svg)](https://crates.io/crates/cgx-cli)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Graph](https://img.shields.io/badge/cgx-live%20graph-blue)](https://aayushbahukhandi.github.io/cgx/)

[**Live Demo**](https://aayushbahukhandi.github.io/cgx/) · [**Documentation**](https://docs.rs/cgx-cli) · [**Releases**](https://github.com/AayushBahukhandi/cgx/releases)

</div>

---

> 🚀 **v0.5.1** — bisect-friendly git hooks: the cgx-managed `post-checkout` / `post-commit` hooks now skip during `git bisect`, `git rebase`, and `git merge`, so `cgx bisect-script` works inside `git bisect run` without dirtying `AGENTS.md` / `CGX_SKILL.md`. Bisect-script also now exits `125` (skip) on empty predicate files and when `rule_violations_max` is set without `--rule-violations N`, instead of silently passing. [v0.5.1 →](https://github.com/AayushBahukhandi/cgx/releases/tag/v0.5.1)
>
> **v0.5.0** — `cgx docs generate --vault` turns your indexed repo into an Obsidian-ready documentation vault (project overview, dep purposes, unused-dep detection, per-file TL;DR, role classification) · docstring extraction now covers Rust, Go, Java, PHP, Python (was TypeScript-only) · `cgx bisect-script` plugs into `git bisect run` to find commits that break declarative graph predicates · new `GraphDb` queries: `get_file_summary`, `get_public_api`, `list_entry_points`, `get_cross_cluster_deps`. [Release notes →](https://github.com/AayushBahukhandi/cgx/releases/tag/v0.5.0)
>
> Earlier: [v0.4.0](https://github.com/AayushBahukhandi/cgx/releases/tag/v0.4.0) — `cgx watch`, `cgx query context`, Claude Code PreToolUse hook, cache management.

---

## A codebase has two graphs

```
   STRUCTURAL                         TEMPORAL
   what calls what                    what changes together

   AuthService                        payments.ts ─┐
       │                                            │ 87% co-change
       ├── login()                                  │ (no import edge!)
       │     └── db.query()           flags.ts  ────┘
       └── logout()                          ▲
             └── session.clear()             │
                                       hidden coupling
                                       lives here
```

Every other code-analysis tool only shows you the left side. cgx builds both — by parsing your AST with Tree-sitter **and** overlaying your full Git history. The temporal graph is where most bugs and refactor risk actually live.

---

## Quick Start

```bash
# 1. Install
brew install aayushbahukhandi/cgx/cgx       # or: cargo install cgx-cli

# 2. Index your repo (functions, imports, git history — all in seconds)
cd your-project && cgx analyze

# 3. Open the WebGL graph in your browser
cgx view --web

# 4. Ask architectural questions in milliseconds
cgx query blast-radius "AuthService"        # what breaks if I change this?
cgx hotspots                                # high churn × coupling = danger zone

# 5. Wire up your AI editor (Cursor, Claude Code, Windsurf, Codex)
cgx setup
```

After `cgx analyze`, two files land in your repo root: `CGX_SKILL.md` (instructs your AI to query the graph instead of opening files) and `AGENTS.md` (prose architecture summary). Both auto-regenerate on every commit.

---

## Generating a documentation vault (new in v0.5)

```bash
cgx docs generate --vault           # writes into your Obsidian vault (auto-detected or configured)
cgx docs generate --local           # writes into ./cgx-docs/
cgx docs generate --vault --force   # full rebuild
cgx docs generate --vault --incremental   # only regenerate files whose graph slice changed
cgx docs status                     # show what would regenerate
cgx docs prompts --next             # stream unfilled AI prose stubs into Claude/Cursor
```

The vault has a layered structure built straight from your graph:

```text
cgx-docs/
├── README.md                       ← project description + dep summary + nav
├── 00-Overview/
│   ├── Architecture.md             ← stack, languages, dep purposes + unused-dep detection,
│   │                                 files-by-role, largest groups, entry points
│   ├── HowToNavigate.md            ← reading path for new contributors
│   └── Glossary.md                 ← node kinds + communities
├── 10-PublicAPI/<group>.md         ← exported symbols per source directory
├── 20-Architecture/
│   ├── Groups.md                   ← primary navigation: by source directory
│   ├── Communities.md              ← raw Louvain clusters
│   ├── CrossClusterDeps.md
│   └── EntryPoints.md
├── 30-Modules/<group>/<file>.md    ← per-file: TL;DR + role badge + structure table with
│                                     inline docstring descriptions + callers/callees +
│                                     tests + ownership + `<!-- cgx-prompt -->` AI stub
├── 40-Risk/
│   ├── Hotspots.md
│   ├── ComplexityHigh.md
│   ├── DeadCode.md
│   └── Duplicates.md
└── 50-Ownership/
    ├── Owners.md
    └── BlameGraph.md
```

cgx itself never calls an LLM. Each module note ends with a self-contained `<!-- cgx-prompt -->` block that bundles every fact your AI needs to write prose (exported symbols, callers/callees, tests, owners, metrics, existing docstrings). Paste it into Claude/Cursor — your schedule, your API key, your cost.

Configure the default vault in `.cgx/config.toml`:

```toml
[docs]
vault_path = "/Users/you/Documents/Obsidian Vault"   # used by --vault
output_dir = "./cgx-docs"                             # used by --local
wiki_links = "obsidian"                               # or "markdown"
prompt_packets = true
frontmatter = true
```

---

## git bisect on the graph (new in v0.5)

`cgx bisect-script` plugs into `git bisect run` and evaluates declarative predicates over the freshly-indexed graph at each commit:

```bash
# 1. Generate a starter predicate file
cgx bisect-script --example > .cgx/bisect.toml

# 2. Edit .cgx/bisect.toml — anything you can express against the graph:
#    node_count_min, nodes_exist, nodes_missing, nodes_alive, rule_violations_max

# 3. Bisect
git bisect start
git bisect bad HEAD
git bisect good v0.4.1
git bisect run cgx bisect-script --analyze
```

Exit codes: `0` = good, `1` = bad, `125` = skip — exactly what `git bisect run` expects. Use `--analyze` to re-index on every commit visited.

---

## Why it matters

| | cgx | Reading source files |
|---|---|---|
| Answer "what breaks if I change `AuthService`?" | `cgx query blast-radius AuthService` → ~50 tokens, 0.3s | open 40 files → 15,000–50,000 tokens |
| Architecture overview for an AI agent | `get_repo_summary` → **~150 tokens** | recursive ls + reads → 50,000+ tokens |
| Find hidden coupling | `cgx hotspots` → co-change scores | grep-and-pray |
| One-shot symbol briefing | `cgx query context login` → **~400 tokens** | 2–15K per file |

---

## Highlighted Features

| Feature | What it does |
|---|---|
| **`cgx docs generate --vault`** | Generate a layered, Obsidian-ready documentation vault from the graph — project overview, per-file TL;DR + role badge, public APIs, hotspots, dep purposes with unused-dep detection, plus AI prose stubs you fill in on your schedule |
| **`cgx bisect-script`** | Drop into `git bisect run` to bisect on declarative graph predicates (node exists, count bounds, no dead-code, ...). Exits 0/1/125 — git does the binary search |
| **Tree-sitter AST parsing** | TS/TSX, JS/JSX, Python, Rust, Go, Java, PHP — parsed in parallel, with docstring extraction across **all** five languages |
| **Git history overlay** | Churn scores, co-change edges, ownership — the temporal graph |
| **`cgx query blast-radius`** | Direct + transitive callers with risk scoring |
| **`cgx watch`** | Debounced live re-indexing on every save |
| **`cgx query context <sym>`** | Callers + deps + community + risk in one ~400-token block |
| **MCP server (10 tools)** | Cursor, Claude Code, Windsurf, Codex — `_summary` field on every response |
| **Claude Code PreToolUse hook** | Auto-injects file context before every Edit/Write |
| **WebGL graph + share links** | Sigma.js renders thousands of nodes; `cgx share` → no-install gist viewer |

<details>
<summary><strong>See the full feature list (30+)</strong></summary>

| Feature | Description |
|---|---|
| **Documentation Vault** | `cgx docs generate --vault` writes a layered Obsidian vault with project overview, per-file TL;DR + role classification, public APIs grouped by directory, hotspots/dead code, dependency table with curated purposes and unused-dep flags, and AI prose stubs you fill on demand |
| **Bisect Script** | `cgx bisect-script` evaluates declarative graph predicates (node exists / missing, count bounds, dead-code) and exits 0/1/125 for `git bisect run` |
| **Multi-language docstring extraction** | Rust (`///`, `/** */`, including across `#[derive]`), Go (`// SymbolName`), Java/PHP (`/** */`, skipping annotations), Python (triple-quoted body docstring), TypeScript — all extracted at parse time |
| **AST Parsing** | Tree-sitter parses TS/TSX, JS/JSX, Python, Rust, Go, Java, PHP in parallel |
| **JSX Caller Tracking** | React component usages (`<MyComp />`) are tracked as call edges |
| **JSX Comment Extraction** | `{/* TODO */}` expression comments and commented-out JSX code are extracted and tagged separately from code comments |
| **Annotation Index** | `cgx todos` lists all TODO/FIXME/HACK/NOTE/BUG/OPTIMIZE/WARN/XXX tags with `comment_type` (code vs jsx) |
| **Docs Coverage** | `cgx docs coverage` reports what % of exported functions have doc comments, by community |
| **Complexity Scoring** | `cgx complexity` ranks functions by cognitive complexity score (if/for/switch/ternary nesting) |
| **Test Coverage Overlay** | `cgx test coverage` / `cgx test gaps` maps test files → source functions via TESTS edges |
| **Dependency Health** | `cgx deps health` parses package.json / Cargo.toml / requirements.txt / go.mod, cross-references OSV for CVEs |
| **PR Review Assistant** | `cgx review` generates a structured brief: blast radius, hotspot alerts, missing tests, suggested reviewers |
| **Architecture Rules** | `cgx rules check` runs SQL or built-in rules (`no_cycles`, `max_coupling`, etc.) with GitHub Actions output |
| **Duplicate Detection** | `cgx dupes` finds near-identical function bodies via normalized AST fingerprints + Jaccard similarity |
| **Architecture Explainer** | `cgx explain AuthService` / `cgx explain --onboard` generates structured Markdown docs from the graph |
| **Timeline** | `cgx timeline` snapshots the graph at each commit; scrubber in the web UI lets you watch architecture evolve |
| **Git Intelligence** | Churn scores, co-change edges, ownership — the temporal graph |
| **DuckDB Storage** | Zero-server embedded graph database. Instant queries. |
| **Community Detection** | Leiden algorithm auto-clusters your codebase into modules |
| **Terminal TUI** | Force-directed graph in Ratatui. Works over SSH. |
| **WebGL Browser Graph** | Sigma.js renders thousands of nodes at 60fps |
| **AI Chat** | Ask questions about your code in natural language. Ollama supported. |
| **MCP Server** | 10 typed tools for Cursor, Claude Code, Windsurf, Codex/OpenCode |
| **Skills System** | `CGX_SKILL.md` auto-generated — works in any AI assistant; `/cgx` slash command in Claude Code via `cgx setup` |
| **Build artifact exclusion** | `web-ui-dist/`, `.next/`, `coverage/`, `*.min.js` etc. excluded automatically; customizable via `.cgxignore` |
| **Share Links** | `cgx share` uploads your graph to a Gist — anyone views it in a browser, no install needed |
| **GitHub Pages Publish** | `cgx publish` pushes a self-contained graph site to your `gh-pages` branch |
| **Graph Diff** | See how your architecture changed between commits |
| **Dead Code Detection** | Five categories: unreferenced exports, unreachable private functions, unused variables, disconnected nodes, zombie files — with High/Medium/Low confidence and false-positive hints |
| **Self-contained binary** | Web UI is embedded in the binary — Homebrew and `cargo install` work out of the box |
| **Live Re-indexing** | `cgx watch` watches your repo and runs debounced incremental analysis on every file change |
| **Agent Context Briefing** | `cgx query context <symbol>` returns callers + deps + community + risk in one ~400-token block (vs 2–15K to read a file). Supports `--json`. |
| **Claude Code Hook** | `cgx setup --hooks` installs a PreToolUse hook that auto-injects file context before every Edit/Write |
| **Cache Management** | `cgx clean --orphaned` sweeps stale db files; `cgx clean --budget 2G` does LRU eviction. Opt-in auto-eviction via `CGX_MAX_CACHE_BYTES` |

</details>

---

## Installation

### Homebrew (macOS / Linux) — recommended

```bash
brew install aayushbahukhandi/cgx/cgx
```

> Tap: [AayushBahukhandi/homebrew-cgx](https://github.com/AayushBahukhandi/homebrew-cgx)

To update:

```bash
brew upgrade aayushbahukhandi/cgx/cgx
```

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

Download the latest release from [GitHub Releases](https://github.com/AayushBahukhandi/cgx/releases/latest). Replace `VERSION` with the tag shown on that page (e.g. `v0.4.0`).

```bash
# macOS arm64 (Apple Silicon)
curl -L https://github.com/AayushBahukhandi/cgx/releases/latest/download/cgx-VERSION-aarch64-apple-darwin.tar.gz | tar xz
sudo mv cgx /usr/local/bin/

# macOS x86_64 (Intel)
curl -L https://github.com/AayushBahukhandi/cgx/releases/latest/download/cgx-VERSION-x86_64-apple-darwin.tar.gz | tar xz
sudo mv cgx /usr/local/bin/

# Linux x86_64
curl -L https://github.com/AayushBahukhandi/cgx/releases/latest/download/cgx-VERSION-x86_64-unknown-linux-gnu.tar.gz | tar xz
sudo mv cgx /usr/local/bin/
```

Windows: download the `.zip` from the releases page and place `cgx.exe` in a directory on your `%PATH%`.

### Verify

```bash
cgx --version
cgx doctor      # checks your setup and editor integrations
```

### Staying up to date

Starting from **v0.1.6**, cgx checks for updates once a day and prints a notice when a newer version is available. You can also check and update manually:

```bash
cgx update          # show installed version, latest version, and upgrade instructions
cgx update --auto   # detect your install method and upgrade automatically
```

Set `CGX_NO_UPDATE_CHECK=1` to disable the background check.

<details>
<summary>Upgrade notes for older versions</summary>

> **On v0.5.0?** Run `cgx update --auto` or reinstall. v0.5.1 fixes the cgx-managed git hooks so they no-op during `git bisect` / `rebase` / `merge` (previously `git bisect run cgx bisect-script` would dirty `AGENTS.md` and `CGX_SKILL.md` at every step and break the next checkout). `cgx bisect-script` also now exits `125` (skip) instead of silently passing when the predicate file is empty or when `rule_violations_max` is set without `--rule-violations N`. **Re-run `cgx analyze` once after upgrading** so the new hook template is installed.
>
> **On v0.4.x?** Run `cgx update --auto` or reinstall. v0.5.0 adds `cgx docs generate --vault` (Obsidian documentation vault with project overview, per-file TL;DR, role classification, dep purposes + unused-dep detection, AI prose stubs), `cgx bisect-script` (drop into `git bisect run`), docstring extraction for Rust/Go/Java/PHP/Python (was TypeScript-only), and four new `GraphDb` query methods. Re-run `cgx analyze` after upgrading to populate the new `doc_comment` data.
>
> **On v0.3.0?** Run `cgx update --auto` or reinstall. v0.3.1 fixes `cgx complexity --combined` (now uses file-level churn), adds `cgx test coverage --by=community`, improves `cgx todos` empty-result messages, shows available built-in rules in `cgx rules list`, and adds a stale-index warning to `cgx complexity`. Also adds previously undocumented commands: `cgx impact`, `cgx init`, `cgx list`, `cgx query deps`, `cgx query community`.
>
> **On v0.2.x?** Run `cgx update --auto` or reinstall. v0.3.0 adds `cgx todos`, `cgx docs coverage`, `cgx complexity`, `cgx test coverage/gaps`, `cgx deps health`, `cgx review`, `cgx rules check`, `cgx dupes`, `cgx explain`, and `cgx timeline` — a full suite of advanced code intelligence commands. Re-run `cgx analyze --force` after upgrading to refresh the graph with new columns (complexity, doc_comment, is_tested, test_count).
>
> **On v0.1.9 or older?** Run `cgx update --auto` or reinstall. v0.2.0 added dead code detection (`cgx query dead-code`) and fixed cross-file call tracking — `new ClassName()`, static method calls, and module-level function calls now all create proper edges, eliminating the majority of false positives.

</details>

---

## AI Integration

### Method 1 — Skills (works everywhere, zero config)

After `cgx analyze`, a `CGX_SKILL.md` file appears in your repo root. Any AI assistant that can read files and run terminal commands — Claude Code, Cursor, GitHub Copilot Chat, Gemini CLI — will automatically use it.

The skill file tells your AI:
- When to call `cgx query` instead of reading source files
- Both MCP tool names (`get_blast_radius`) and CLI equivalents (`cgx query blast-radius`) in one place
- Live stats about your codebase (hotspots, communities, entry points)
- Mandatory trigger language so the AI fires cgx without being asked

**Result:** Your AI stops reading source files to answer an architectural question and runs one `cgx query` command instead. `get_repo_summary` costs ~150 tokens; reading a single large source file costs 15,000–50,000. Same answer, without opening a file.

### Method 1b — `/cgx` slash command in Claude Code

```bash
cgx setup   # installs ~/.claude/skills/cgx/SKILL.md + registers /cgx
```

After running `cgx setup`, type `/cgx` in Claude Code to analyze any repo interactively — even repos that haven't been pre-indexed. Works like `/graphify` but for structural code queries rather than knowledge graph building.

### Method 2 — MCP Server (Cursor, Claude Code, Windsurf, Codex/OpenCode)

```bash
cgx setup    # auto-detects your editors and writes their MCP configs
```

Restart your editor. cgx now exposes 10 typed tools your AI can call directly:

| Tool | What it answers | Typical tokens |
|---|---|---|
| `get_repo_summary` | Architecture overview: nodes, communities, hotspots, god nodes | ~150 |
| `find_symbol` | Where is X defined? File + line | ~50 |
| `get_neighbors` | What does X depend on? What depends on X? | ~50 |
| `get_blast_radius` | What breaks if I change X? Risk level + affected count | ~50 |
| `get_call_chain` | Trace from A to B through the call graph | ~100 |
| `get_community` | All nodes in the auth/db/payments cluster | ~200 |
| `search_graph` | Full-text search over all symbol names | ~100 |
| `get_hotspots` | Highest churn × coupling files | ~100 |
| `get_file_owners` | Git blame ownership for any file | ~50 |
| `get_dead_code` | Unreferenced exports, unreachable functions, zombie files — with confidence + false-positive hints | ~100 |
| `run_query` | Raw SQL SELECT against the graph (read-only) | varies |

Every response includes a `_summary` field — a plain-text sentence the model reads first before parsing JSON, so it can skip deeper inspection when not needed.

**Example:** Ask "refactor the login function to add rate limiting" in Claude Code. It calls `get_blast_radius`, `get_neighbors`, and `get_file_owners` — 3 tool calls, under **200 tokens total** — then writes the code knowing exactly what it needs to update.

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
| Cognitive complexity scoring | ✅ | ❌ | ❌ |
| TODO/FIXME annotation index | ✅ | ❌ | ❌ |
| Doc coverage reporting | ✅ | ❌ | ❌ |
| Test coverage overlay | ✅ | ❌ | ❌ |
| Dependency CVE health | ✅ | ❌ | ❌ |
| PR review brief generator | ✅ | ❌ | ❌ |
| Architecture fitness rules | ✅ | ❌ | ❌ |
| Duplicate / clone detection | ✅ | ❌ | ❌ |
| Architecture explainer | ✅ | ❌ | ❌ |
| Commit timeline snapshots | ✅ | ❌ | ❌ |
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
| Session context overhead | ~1,300 tokens | unknown | ~15,000 tokens |
| `get_repo_summary` cost | ~150 tokens | n/a MCP | no MCP |
| License | MIT | Non-commercial | MIT |

---

## Demo videos

<table>
<tr>
<td align="center" width="50%">

**CLI**

[![cgx CLI demo](https://aayushbahukhandi.github.io/cgx/thumb-cli.jpg)](https://aayushbahukhandi.github.io/cgx/cgx-cli.mp4)

</td>
<td align="center" width="50%">

**Web UI**

[![cgx Web UI demo](https://aayushbahukhandi.github.io/cgx/thumb-web.jpg)](https://aayushbahukhandi.github.io/cgx/cgx-web.mp4)

</td>
</tr>
</table>

---

## Core Commands

### Analysis

```bash
cgx analyze                        # index current repo
cgx analyze ./path                 # index any local path
cgx analyze github:owner/repo      # clone from GitHub and index (stored in ~/.cgx/clones/)
cgx analyze --watch                # live-reload on file save
cgx analyze --incremental          # re-parse only changed files (used by git hooks)
cgx analyze --no-git               # skip git history layer
cgx analyze --force                # full clean re-index
cgx analyze --verbose              # verbose output during analysis
cgx analyze --no-cluster           # skip community detection
cgx analyze --quiet                # suppress output
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
# To unshare: gh gist delete <gist-id>   (shown in cgx share output)
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
cgx query deps "AppError"               # dependencies of a node
cgx query blast-radius "deleteUser"     # what breaks if this changes?
cgx query chain "login -> AppError"     # trace a call chain (format: "from -> to")
cgx query community 7                   # all nodes in community #7
cgx query dead-code                     # unreferenced exports, unreachable functions, zombie files
cgx query dead-code --kind=exports      # filter: exports | functions | variables | files | disconnected
cgx query dead-code --confidence=high   # only high-confidence candidates
cgx query dead-code --summary           # count table by category and confidence
cgx query search "session"             # search by symbol name
cgx query owners src/payments/index.ts  # git blame ownership for a file
```

### Git Intelligence

```bash
cgx hotspots                   # high churn × high coupling = danger zone
cgx blame-graph                # ownership by contributor
cgx diff HEAD~5                # architecture diff between commits
cgx impact                     # downstream impact of changes in the last 7 days
cgx impact --since=14          # look back N days
```

### Documentation & Annotations

```bash
cgx todos                                    # list all TODO/FIXME/HACK/NOTE tags
cgx todos --kind=FIXME                       # filter by tag kind
cgx todos --comment-type=jsx                 # only JSX {/* */} comments
cgx todos --comment-type=jsx_commented_code  # JSX code blocks that are commented out
cgx todos --json                             # output as JSON
cgx docs coverage                            # % of exported functions with doc comments
```

### Complexity

```bash
cgx complexity                     # top 20 functions by cognitive complexity
cgx complexity --threshold=0.15    # functions with score > 0.15
cgx complexity --combined          # sort by complexity × churn combined risk
```

### Test Coverage

```bash
cgx test coverage                  # overall test coverage stats
cgx test coverage --by=community   # breakdown by cluster
cgx test gaps                      # untested high-coupling functions
cgx test suggest                   # prioritized test-writing list
```

### Dependency Health

```bash
cgx deps health                    # full report — packages, versions, CVE counts
cgx deps health --critical         # only CVE-affected packages
cgx deps audit                     # fast CVE count per package
cgx deps outdated                  # packages with newer versions available
```

### PR Review Assistant

```bash
cgx review                         # current branch vs main/master
cgx review feature/my-branch       # specific branch vs main
cgx review HEAD~5                  # last 5 commits
cgx review --format=markdown       # GitHub PR comment format
cgx review --format=github-actions # GitHub Actions annotation format
```

### Architecture Rules

```bash
cgx rules check                    # run all rules in .cgx/rules.toml
cgx rules check --rule=no-cycles   # run a specific rule
cgx rules list                     # list all defined rules
```

Example `.cgx/rules.toml`:

```toml
[[rules]]
name = "no-circular-dependencies"
built_in = "no_cycles"
severity = "error"

[[rules]]
name = "no-direct-db-access-outside-repository-layer"
description = "Only repository files should import from db/"
severity = "error"
query = """
SELECT n.path, e.dst FROM edges e
JOIN nodes n ON n.id = e.src
WHERE e.kind = 'IMPORTS' AND e.dst LIKE '%/db/%'
  AND n.path NOT LIKE '%/repository/%'
"""
```

Built-in rules: `no_cycles`, `max_coupling`, `max_complexity`, `require_docs_for_public`.

### Duplicate Detection

```bash
cgx dupes                          # all clone pairs (threshold 80%)
cgx dupes --threshold=0.9          # near-identical clones only
cgx dupes --kind=exact             # exact duplicates only
```

### Architecture Explainer

```bash
cgx explain AuthService            # explain a specific symbol
cgx explain src/auth/              # explain a folder
cgx explain --onboard              # full onboarding guide for the repo
cgx explain --onboard --out=ARCHITECTURE.md  # write to file
```

### Timeline

```bash
cgx timeline                       # snapshot last 20 commits
cgx timeline --commits=50          # last N commits
cgx timeline --since=2024-01       # since a date
cgx timeline --json                # output as JSON
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
cgx init                       # create .cgx/config.toml with defaults
cgx init --yes                 # non-interactive with defaults
cgx list                       # list all indexed repos with node/edge counts
```

---

## AI Chat

The browser UI (`cgx view --web`) includes a built-in chat panel. Ask natural language questions about your codebase.

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

```bash
cgx serve                              # start server (auto-opens browser)
cgx serve --no-open                    # start server without opening browser
```

> **Privacy note:** cgx chat sends only graph metadata to the AI — node names, file paths, churn scores, community labels. It never sends your source code. With Ollama, nothing leaves your machine.

---

## Supported Languages

| Language | Parser | Status |
|---|---|---|
| TypeScript / TSX | tree-sitter-typescript | ✅ Stable — incl. JSX comment extraction |
| JavaScript / JSX | tree-sitter-javascript | ✅ Stable — incl. JSX comment extraction |
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

### Excluding paths from analysis

cgx automatically skips build artifacts: `node_modules/`, `target/`, `dist/`, `*-dist/` (e.g. `web-ui-dist/`), `.next/`, `coverage/`, `vendor/`, `venv/`, `*.min.js`, `*.bundle.js`, and similar.

For custom exclusions, create `.cgxignore` in your repo root — same syntax as `.gitignore`:

```gitignore
# .cgxignore
generated/
proto/
vendor/
*_pb.ts
```

`.cgxignore` files in subdirectories also work (scoped to that subtree).

### Advanced configuration

For further customization, create `.cgx/config.toml` in your repo root:

```toml
[analyze]
# Languages to parse (default: all supported)
languages = ["typescript", "javascript", "python"]

# Directories to skip (prefer .cgxignore for per-repo exclusions)
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

The graph is stored locally at `~/.cgx/repos/<hash>.db` — one DuckDB file per repo. No external services, no cloud, no network required for local use.

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

**v0.4.0 — Live Indexing & Agent Workflow (shipped)**
- [x] `cgx watch` — top-level watch command with debounced incremental re-analyze on file changes (`--debounce-ms` tunable)
- [x] `cgx query context <symbol>` — one-shot agent briefing: callers + deps + community + risk in ~400 tokens, with `--json` for tool consumption
- [x] `cgx hook` + `cgx setup --hooks` — opt-in Claude Code PreToolUse hook that injects file context on Edit/Write/MultiEdit
- [x] `cgx clean --orphaned` — sweeps stale registry entries AND unreferenced `.db` files in `~/.cgx/repos/`
- [x] `cgx clean --budget <SIZE>` — LRU eviction (e.g. `--budget 2G`); opt-in auto-eviction via `CGX_MAX_CACHE_BYTES` env var
- [x] `RepoEntry.last_used_at` — touched on every query for LRU tracking
- [x] Spinner UX during analyze phases (walk / parse / resolve / store / git / cluster) via indicatif
- [x] Colored "⚡ Update available" notice via `console::style`
- [x] Bug fix: incremental analyze no longer double-upserts edges (resolved DuckDB ART index error on large repos)
- [x] Bug fix: `cgx doctor` hints at `cgx clean --orphaned` when orphaned registry entries are detected

<details>
<summary><strong>Previous releases (v0.3.x and earlier)</strong></summary>

**v0.3.2 — Bug Fixes (shipped)**
- [x] `cgx share` — fixed: viewer now loads the shared graph instead of the published cgx graph when `?data=` URL param is present
- [x] `cgx publish` — fixed: crash on repos with subdirectories in `assets/` (invalid git tree entry)
- [x] `cgx share` — now prints `gh gist delete <id>` in output so users know how to unshare

**v0.3.1 — Bug Fixes & Documentation (shipped)**
- [x] `cgx complexity --combined` — fixed: now correctly uses file-level churn (not always-zero function churn)
- [x] `cgx test coverage --by=community` — implemented: community-level test coverage breakdown table
- [x] `cgx todos` empty-result messages — fixed: "No FIXME annotation comments found." instead of "Run cgx analyze"
- [x] `cgx complexity` stale warning — warns when all scores are 0.00 and suggests `--force` re-index
- [x] `cgx rules list` — now shows all 4 available built-in rules even without a `.cgx/rules.toml`
- [x] `cgx query chain` format — README corrected to `"A -> B"` format
- [x] `cgx query search` — clarified as symbol-name search (not full-text code search)
- [x] `cgx impact`, `cgx init`, `cgx list`, `cgx query deps`, `cgx query community` — documented in README

**v0.3.0 — Advanced Code Intelligence (shipped)**
- [x] `cgx todos` — annotation index (TODO/FIXME/HACK/NOTE/BUG/OPTIMIZE/WARN/XXX, JSX comments)
- [x] `cgx docs coverage` — documentation coverage by community
- [x] `cgx complexity` — cognitive complexity scoring per function
- [x] `cgx test coverage` / `cgx test gaps` — test coverage overlay via TESTS edges
- [x] `cgx deps health` / `cgx deps audit` — dependency CVE health (OSV API)
- [x] `cgx review` — PR review brief (blast radius, hotspots, missing tests, reviewers)
- [x] `cgx rules check` — architecture fitness functions (SQL + built-in rules)
- [x] `cgx dupes` — duplicate/clone detection via normalized AST fingerprints
- [x] `cgx explain` — architecture explainer for symbols, folders, and full onboarding
- [x] `cgx timeline` — git commit timeline snapshots

</details>

**Next**
- [ ] `cgx changelog` — generate changelogs from graph diffs
- [ ] VS Code extension
- [ ] Per-query token budget (`cgx query <cmd> --budget=tokens` with importance-ranked truncation)
- [ ] Mermaid diagram auto-commit to docs/ on every push (GitHub Action)
- [ ] Ruby, Swift, C/C++ parsers
- [ ] Semantic code search (`cgx query search --semantic` with optional LLM summaries)
- [ ] cgx cloud — shared graphs for teams (hosted)

---

## License

MIT — use it in anything, commercial or otherwise.

---

<div align="center">

Built with Rust 🦀 · Tree-sitter · DuckDB · Sigma.js

**[Star this repo](https://github.com/AayushBahukhandi/cgx) if cgx saves you time.**

</div>
