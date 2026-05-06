# cgx-cli

Command-line interface for [cgx](https://github.com/AayushBahukhandi/cgx) — turn any Git repository into a queryable knowledge graph.

## Installation

```bash
cargo install cgx-cli
```

The binary is named `cgx`.

## Quick Start

```bash
# Index your repo
cd your-project
cgx analyze

# Explore in terminal
cgx view

# Explore in browser
cgx view --web

# Find risky files
cgx hotspots

# Query the graph
cgx query blast-radius "AuthService"
cgx query find "login" --kind=Function
cgx query dead-code

# Export
cgx export --format=mermaid
cgx export --format=json

# Publish to GitHub Pages
cgx publish
```

## Commands

| Command | Description |
|---------|-------------|
| `cgx analyze` | Index current repo into DuckDB graph |
| `cgx view` | Terminal TUI (Ratatui) |
| `cgx view --web` | Launch browser graph (WebGL) |
| `cgx serve` | HTTP API server for web UI |
| `cgx query` | Query subcommands: find, deps, blast-radius, chain, owners, search, community, dead-code |
| `cgx hotspots` | High churn × high coupling files |
| `cgx blame-graph` | Ownership by contributor |
| `cgx diff` | Architecture diff between commits |
| `cgx impact` | Downstream impact of recent changes |
| `cgx export` | JSON, Mermaid, DOT, SVG, GraphML |
| `cgx publish` | Push graph to GitHub Pages |
| `cgx setup` | Auto-configure MCP for Cursor, Claude Code, etc. |
| `cgx mcp` | Start MCP stdio server |
| `cgx doctor` | Run diagnostics |
| `cgx clean` | Remove indexed data |
| `cgx update` | Check for updates |

## AI Chat

The browser UI includes a built-in chat panel powered by:

- **OpenAI** (GPT-4o-mini) — set `OPENAI_API_KEY`
- **Anthropic** (Claude Haiku) — set `ANTHROPIC_API_KEY`, `CGX_CHAT_PROVIDER=anthropic`
- **Ollama** (local) — `ollama pull codellama && cgx serve`
- **Any OpenAI-compatible API** — Together, Fireworks, Groq, etc.

```bash
export OPENAI_API_KEY=sk-...
cgx serve
```

## License

MIT
