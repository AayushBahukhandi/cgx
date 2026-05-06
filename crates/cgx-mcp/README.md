# cgx-mcp

MCP (Model Context Protocol) server for [cgx](https://github.com/AayushBahukhandi/cgx).

## What is MCP?

MCP is a protocol that lets AI editors (Cursor, Claude Code, Windsurf, etc.) call tools directly. Instead of your AI reading 40 files to understand your codebase, it calls `cgx` tools that query the pre-built knowledge graph in milliseconds.

## Tools exposed

| Tool | What it answers |
|------|----------------|
| `get_repo_summary` | Full architectural overview |
| `find_symbol` | Where is X defined? |
| `get_neighbors` | What does X depend on? |
| `get_blast_radius` | What breaks if I change X? |
| `get_call_chain` | Trace from A to B |
| `get_community` | All nodes in a cluster |
| `search_graph` | Full-text search over symbols |
| `get_hotspots` | Highest churn × coupling |
| `get_file_owners` | Git blame ownership |
| `run_query` | Raw SQL SELECT against DuckDB |

## Usage

This crate is not used directly by end users. The `cgx-cli` crate starts it automatically:

```bash
cgx mcp    # starts MCP stdio server
cgx setup  # auto-configures editors to use it
```

## For AI Editor Developers

The server speaks JSON-RPC 2.0 over stdio. See `src/server.rs` for the protocol implementation.

## License

MIT
