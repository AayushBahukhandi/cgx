# cgx launch drafts

Channels in priority order: Twitter/X → Reddit (4 subs, 2hr apart) → LinkedIn → Hacker News.

Before posting anywhere: GitHub repo description + topics MUST be set (currently empty).

---

## GitHub repo settings (do this first — 2 minutes)

**Description:**
> Turn any Git repo into a queryable knowledge graph. Tree-sitter + git history + DuckDB + WebGL graph. MCP server for Cursor & Claude Code.

**Topics:**
`rust` `cli` `code-analysis` `tree-sitter` `knowledge-graph` `developer-tools` `mcp` `duckdb` `ai` `ast` `git-history`

You can set both at once via the GitHub CLI:

```bash
gh repo edit AayushBahukhandi/cgx \
  --description "Turn any Git repo into a queryable knowledge graph. Tree-sitter + git history + DuckDB + WebGL graph. MCP server for Cursor & Claude Code." \
  --add-topic rust --add-topic cli --add-topic code-analysis --add-topic tree-sitter \
  --add-topic knowledge-graph --add-topic developer-tools --add-topic mcp \
  --add-topic duckdb --add-topic ai --add-topic ast --add-topic git-history
```

---

## Twitter / X thread

Post as a thread. First tweet is GIF-only — no caption text on the image, the GIF carries the message. Schedule for 9–11am ET on a Tue/Wed/Thu.

### Tweet 1 (the hook — GIF only, minimal text)

> [attach: 20-second GIF of `cgx analyze` → `cgx view --web` spinning up]
>
> I built a tool that turns any Git repo into a queryable knowledge graph.
>
> Written in Rust. Open source. 🧵

### Tweet 2 (the thesis)

> Every code-analysis tool only shows you the structural graph — what imports what.
>
> But codebases have a second graph: what *changes together* in git history.
>
> Two files that never import each other but co-change in 87% of commits = hidden coupling. That's where bugs live.
>
> cgx shows you both.

### Tweet 3 (the AI angle — the killer number)

> The other thing this fixes: AI assistants burning tokens reading source files to answer architectural questions.
>
> Reading one large source file = 15,000–50,000 tokens.
> `cgx get_repo_summary` = ~150 tokens. Same answer.
>
> Built-in MCP server. Works with Cursor, Claude Code, Windsurf, Codex.

### Tweet 4 (the install + link)

> One command to install. One command to index any repo. Self-contained binary, no LLM required.
>
> ```
> brew install aayushbahukhandi/cgx/cgx
> cgx analyze
> cgx view --web
> ```
>
> github.com/AayushBahukhandi/cgx
>
> #rustlang #opensource #devtools

### Quote-tweet bait (a 5th, posted ~3 hours later if engagement is hot)

> A few people asked how it works under the hood:
>
> Tree-sitter for AST → DuckDB for storage → libgit2 for history → Leiden for community detection → Sigma.js (WebGL) for the graph view.
>
> The whole thing fits in a single binary. Web UI is embedded via rust-embed.

---

## Reddit — r/rust

**Title:** I built cgx in Rust — turns any Git repo into a queryable knowledge graph (self-analyzes in 4 seconds)

**Body:**

Hey r/rust,

I've been working on **cgx** for the last several months — a CLI that indexes any Git repository as a graph (functions, classes, imports, calls) and overlays your git history on top. It's all in Rust: Tree-sitter for parsing, DuckDB for storage, libgit2 for history, Leiden for community detection, and the web UI is embedded into the binary via `rust-embed` so `cargo install cgx-cli` gets you everything in one shot.

**The self-dogfood test:** running `cgx analyze` on the cgx repo itself produces a 4-second index, ~1,400 nodes, ~3,200 edges. The CLI then answers architectural questions in milliseconds. Things like:

```
cgx query blast-radius "NodeRow"     # → 47 direct, 102 transitive callers
cgx hotspots                          # top 5 files by churn × coupling
cgx query context "ParseResult"       # 400-token briefing for an AI agent
```

**Why Rust mattered here:**

- Tree-sitter has excellent Rust bindings; parsing 7 languages in parallel is trivial with rayon
- DuckDB's Rust crate makes embedded graph queries fast and zero-config
- The whole thing compiles to a single ~25MB binary with the web UI embedded — Homebrew, `cargo install`, and pre-built binaries all just work
- The MCP server (separate crate, `cgx-mcp`) is a JSON-RPC 2.0 stdio loop in ~600 lines

**What's interesting that I haven't seen elsewhere:**

Most code-graph tools only show you the structural graph. cgx also builds the *temporal graph* from git history — files that always change together even if they don't import each other. That's the hidden coupling that wrecks refactors. Co-change edges + churn scores get you a real ranked list of risky files, not a guess.

Open source, MIT, self-contained binary, no LLM required.

```bash
brew install aayushbahukhandi/cgx/cgx       # or: cargo install cgx-cli
cd your-rust-project && cgx analyze
cgx view --web
```

Repo: https://github.com/AayushBahukhandi/cgx
Live demo (this repo's own graph): https://aayushbahukhandi.github.io/cgx/

Happy to answer anything about the architecture — incremental indexing, DuckDB schema, Leiden tuning, embedding a Vite bundle into a Rust binary, etc.

---

## Reddit — r/programming

**Title:** A codebase has two graphs: the one your imports show you, and the one your git history hides

**Body:**

Every code-analysis tool I've used only shows the structural graph — what calls what, what imports what, right now. That's half the picture.

The other half is the *temporal graph* — what changes together. Two files that never import each other but co-change in 87% of commits aren't independent; they have hidden coupling. That's where the surprises live during refactors.

I built a tool called **cgx** that builds both. Tree-sitter parses every function, class, and import. Then it overlays your full git history to produce churn scores, ownership, and co-change edges. It stores the whole graph locally in DuckDB and answers questions in milliseconds.

A few queries that I use constantly:

```
cgx query blast-radius "AuthService"
  → 14 direct callers, 67 total affected. Risk: HIGH.

cgx hotspots
  → ranked list of files by churn × coupling.
    The top 5 are almost always the ones you'd guess if you'd been here 2 years.

cgx review feature/payments-v2
  → PR review brief: blast radius, missing tests, hotspot alerts, suggested reviewers.
```

The thing I'm most proud of: dead code detection that actually works. Five categories, three confidence levels, with false-positive hints (e.g. "this looks unused but it's exported from a package entry point — likely a public API"). The first version had absurd false positive rates; the current version is the third rewrite and finally produces a list you can act on.

Open source, MIT, runs locally, no LLM required for indexing. Written in Rust, single binary, web UI is embedded.

GitHub: https://github.com/AayushBahukhandi/cgx
Live graph of cgx itself: https://aayushbahukhandi.github.io/cgx/

I'd love feedback — especially from anyone who's tried to build codebase-graph tools before. The co-change scoring threshold is one place I keep tuning.

---

## Reddit — r/LocalLLaMA (or r/MachineLearning)

**Title:** Stop your AI agent from burning 50K tokens reading source files — give it a graph instead (cgx + MCP, works with Ollama)

**Body:**

If you run a local model with Cursor, Claude Code, or any MCP-capable agent, you've probably watched it open 20 files to answer an architectural question. That's 15,000–50,000 tokens *per file*, most of which is irrelevant.

I built **cgx** to fix this. It indexes a repo once (Tree-sitter AST + git history → DuckDB) and exposes 10 typed MCP tools your agent can call instead of opening files:

| Tool | Typical tokens |
|---|---|
| `get_repo_summary` | ~150 |
| `find_symbol` | ~50 |
| `get_blast_radius` | ~50 |
| `get_neighbors` | ~50 |
| `get_call_chain` | ~100 |
| `get_hotspots` | ~100 |
| `get_dead_code` | ~100 |

Every response has a `_summary` field — a plain-text sentence the model reads first so it can decide whether to parse the JSON.

Concrete example: "refactor the login function to add rate limiting." Claude Code (or any MCP client) calls `get_blast_radius`, `get_neighbors`, `get_file_owners` — **3 calls, under 200 tokens total** — then writes the patch knowing exactly what depends on `login`.

**Local-model friendly:** the indexer itself never needs an LLM. The optional in-browser chat panel supports OpenAI, Anthropic, and **Ollama**, plus any OpenAI-compatible endpoint (Together, Groq, vLLM, etc.). With Ollama, nothing leaves your machine.

Self-contained Rust binary, MIT-licensed:

```bash
brew install aayushbahukhandi/cgx/cgx
cgx analyze
cgx setup    # auto-writes MCP configs for Cursor / Claude Code / Windsurf
```

Repo: https://github.com/AayushBahukhandi/cgx

Curious whether anyone here has been wrestling with this same context-budget problem. The token numbers above are real, measured on a mid-size TypeScript repo — happy to share the methodology.

---

## Reddit — r/webdev

**Title:** I made a WebGL visualizer that turns any Git repo into a navigable graph (cgx — share links work without install)

**Body:**

I built a tool called **cgx** that indexes any Git repository — every function, every import, every call — and renders the whole thing as a WebGL graph in the browser. Sigma.js handles tens of thousands of nodes at 60fps. Communities are auto-detected with Leiden clustering so the layout actually makes sense instead of being a hairball.

A couple of things I think you might find useful:

**1. Share links — no install needed on the receiving end.**

```
cgx share
→ https://aayushbahukhandi.github.io/cgx/?data=https://gist.githubusercontent.com/...
```

That URL loads a hosted viewer that pulls the graph JSON from a GitHub Gist. Anyone on your team (or in a PR review, or on a job interview) can open it in their browser. No install. Works on mobile.

**2. `cgx publish` deploys a self-contained graph site to your `gh-pages` branch.**

One command. The site is fully static — Vite-built React + Sigma.js — and gives you a shareable, permanent URL of your architecture at that commit. Nice thing to embed in a README badge.

**3. The web UI is embedded into the CLI binary.**

I built it with Vite + React + Sigma.js + Zustand, then `rust-embed`'s the `dist/` into the Rust binary at compile time. Whole thing is ~25MB. No "go install npm first" — Homebrew or `cargo install` gets you the full UI.

Live demo (cgx visualizing its own codebase): https://aayushbahukhandi.github.io/cgx/

Repo: https://github.com/AayushBahukhandi/cgx

The hardest UI problem was actually edge bundling for repos with thousands of co-change edges — happy to chat about how I ended up handling that.

---

## LinkedIn post

Long-form. Aim for ~250–350 words. Post Tue–Thu morning. Tag two or three people you actually know who'd care; don't tag strangers.

---

Last year I joined a new codebase and it took me three weeks before I felt like I could safely change anything.

Not because the code was bad. Because nothing tells you the *shape* of an unfamiliar codebase. You read README files written 18 months ago by someone who's left, you grep for symbol names, you open 40 files to trace one function, and slowly a fuzzy picture forms in your head.

That experience is what I built **cgx** to fix.

cgx takes any Git repository and turns it into a queryable knowledge graph in one command. Functions, classes, imports, call edges — parsed with Tree-sitter across seven languages. Then it overlays your full Git history: churn scores, ownership, and co-change edges. The result is two graphs in one:

→ **The structural graph** — what calls what
→ **The temporal graph** — what changes together

The temporal one is the interesting one. It surfaces files that never import each other but co-change in 87% of commits — hidden coupling that no static analyzer can see. That's almost always where refactor risk actually lives.

A second thing I noticed building this: AI coding assistants burn enormous amounts of context reading source files just to answer architectural questions. Opening one large file = 15,000–50,000 tokens. cgx's MCP server answers the same question in ~150 tokens because it's querying a graph, not reading source code. Cursor and Claude Code stop opening files and start asking the graph instead.

It's open source (MIT), written in Rust, ships as a single self-contained binary. No LLM required. Works fully offline.

If you've ever joined a new codebase and wished there was a map — this is the map.

GitHub: https://github.com/AayushBahukhandi/cgx
Live demo: https://aayushbahukhandi.github.io/cgx/

If you try it on your own codebase and the hotspots list surprises you, I'd love to hear about it.

---

## Hacker News (Show HN)

**Title:** Show HN: cgx – turn any Git repo into a queryable knowledge graph (Rust)

**URL:** https://github.com/AayushBahukhandi/cgx

**First comment (post immediately after submission, addressed from the author):**

Hi HN — I'm the author.

cgx indexes a Git repository as a graph: Tree-sitter parses every function, class, and import across 7 languages; libgit2 overlays the full git history (churn, ownership, co-change edges); the result is stored in an embedded DuckDB file and queried in milliseconds. The web UI (Sigma.js / WebGL) is embedded into the Rust binary, so `brew install` or `cargo install cgx-cli` gives you the full thing in one shot — no node, no python, no server.

The thesis I keep coming back to: **a codebase has two graphs.** The structural one (what calls what) is what every code-analysis tool shows you. The temporal one — what *changes together* in commits — is the one nobody surfaces, and it's where most refactor risk actually lives. Two files that never import each other but co-change in 87% of commits aren't independent; they have hidden coupling.

The second motivation was the AI-agent angle. Reading one large source file to answer an architectural question costs 15,000–50,000 tokens. cgx exposes an MCP server with 10 typed tools (`get_blast_radius`, `get_neighbors`, `get_repo_summary`…) — `get_repo_summary` is ~150 tokens; the same answer your agent would otherwise pay 50,000 for. Works with Cursor, Claude Code, Windsurf, Codex, and any OpenAI-compatible local model via Ollama.

A few things I'd love feedback on:

- The co-change threshold (default: 2 shared commits). Anything lower produces noise on large repos; anything higher misses real coupling on small repos.
- Dead-code detection across five categories with confidence levels. This is the third rewrite — current false-positive rate feels acceptable on the repos I've tested but I'd love adversarial inputs.
- Language coverage. C/C++, Ruby, and Swift parsers are next. Anyone with a strong opinion on tree-sitter-cpp's quirks, I'm listening.

MIT, no telemetry, no cloud, no LLM required for indexing. Try it on your own repo and tell me what surprises you in the hotspots list.

---

## Posting cadence

| Time (ET) | Channel |
|---|---|
| Tue 9:00am | Twitter thread |
| Tue 11:00am | r/rust |
| Tue 1:00pm | r/programming |
| Tue 3:00pm | r/LocalLLaMA |
| Tue 5:00pm | r/webdev |
| Wed 9:00am | LinkedIn |
| Wed 10:00am | Show HN |

Reply to every comment within the first 2 hours on each platform. HN especially — the front-page algorithm weighs author engagement heavily in the first hour.

---

## Things to NOT do

- Don't post the same body across Reddit subs. Mods notice. Use the four different angles above.
- Don't tag random influencers on Twitter. Tag people you actually know.
- Don't lead any post with "I built this in a weekend!" — readers don't care, and it sandbags polished work.
- Don't link-dump on LinkedIn. The post above is the post; the link is at the bottom.
- Don't ask for stars in the README or the posts. Earn them.
