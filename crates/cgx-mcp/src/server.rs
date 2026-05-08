use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use anyhow::Context;
use cgx_engine::GraphDb;

use crate::tools::handle_tool_call;

#[derive(serde::Deserialize, Debug)]
struct Request {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<serde_json::Value>,
    method: String,
    #[serde(default)]
    params: Option<serde_json::Value>,
}

#[derive(serde::Serialize)]
struct Response {
    #[allow(dead_code)]
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ErrorBody>,
}

#[derive(serde::Serialize)]
struct ErrorBody {
    code: i32,
    message: String,
}

pub fn run(repo_path: &Path) -> anyhow::Result<()> {
    let db = GraphDb::open(repo_path)
        .context("Failed to open graph database — run `cgx analyze` first")?;

    if db.node_count().unwrap_or(0) == 0 {
        eprintln!("cgx-mcp: warning: graph is empty — run `cgx analyze` first");
    }

    let stdin = std::io::stdin();
    let reader = BufReader::new(stdin.lock());
    let mut stdout = std::io::stdout().lock();

    for line in reader.lines() {
        let line = line.context("failed to read stdin line")?;
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let request: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = Response {
                    jsonrpc: "2.0".into(),
                    id: Some(serde_json::Value::Null),
                    result: None,
                    error: Some(ErrorBody {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                    }),
                };
                writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
                stdout.flush()?;
                continue;
            }
        };

        // Notifications have no id — do not respond
        if request.id.is_none() {
            continue;
        }

        let response = handle_request(&request, &db);

        writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
        stdout.flush()?;
    }

    Ok(())
}

fn handle_request(request: &Request, db: &GraphDb) -> Response {
    let id = request.id.clone();

    match request.method.as_str() {
        "initialize" => {
            let result = serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "cgx",
                    "version": env!("CARGO_PKG_VERSION")
                }
            });
            Response {
                jsonrpc: "2.0".into(),
                id,
                result: Some(result),
                error: None,
            }
        }

        "initialized" => Response {
            jsonrpc: "2.0".into(),
            id,
            result: Some(serde_json::json!({})),
            error: None,
        },

        "ping" => Response {
            jsonrpc: "2.0".into(),
            id,
            result: Some(serde_json::json!({})),
            error: None,
        },

        "tools/list" => Response {
            jsonrpc: "2.0".into(),
            id,
            result: Some(tools_list()),
            error: None,
        },

        "tools/call" => {
            let params = request.params.as_ref().and_then(|p| p.as_object());
            let name = params
                .and_then(|p| p.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let arguments = params
                .and_then(|p| p.get("arguments"))
                .cloned()
                .unwrap_or(serde_json::json!({}));

            match handle_tool_call(name, &arguments, db) {
                Ok(text) => {
                    let result = serde_json::json!({
                        "content": [{ "type": "text", "text": text }]
                    });
                    Response {
                        jsonrpc: "2.0".into(),
                        id,
                        result: Some(result),
                        error: None,
                    }
                }
                Err(e) => Response {
                    jsonrpc: "2.0".into(),
                    id,
                    result: None,
                    error: Some(ErrorBody {
                        code: -32000,
                        message: e,
                    }),
                },
            }
        }

        "notifications/initialized" | "notifications/cancelled" => {
            // Should not reach here (skipped before handle_request), but handle gracefully
            Response {
                jsonrpc: "2.0".into(),
                id,
                result: Some(serde_json::json!({})),
                error: None,
            }
        }

        _ => Response {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(ErrorBody {
                code: -32601,
                message: format!("Method not found: {}", request.method),
            }),
        },
    }
}

fn tools_list() -> serde_json::Value {
    serde_json::json!({
        "tools": [
            {
                "name": "get_repo_summary",
                "description": "Get a full architectural overview of the indexed codebase. Always call this first in a new session.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "find_symbol",
                "description": "Find any function, class, variable, or type by name. Supports optional kind filter.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "The symbol name to search for" },
                        "kind": { "type": "string", "description": "Optional kind filter: Function, Class, File, Type, Variable" }
                    },
                    "required": ["name"]
                }
            },
            {
                "name": "get_neighbors",
                "description": "Get direct dependencies of a node (nodes that it calls or imports).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "node_id": { "type": "string", "description": "Full node ID (e.g., fn:src/auth.ts:login)" },
                        "depth": { "type": "integer", "description": "How many hops (1-3, default 1)" }
                    },
                    "required": ["node_id"]
                }
            },
            {
                "name": "get_call_chain",
                "description": "Trace a call path between two symbols. Returns the path if found.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "from": { "type": "string", "description": "Source node ID or name" },
                        "to": { "type": "string", "description": "Target node ID or name" }
                    },
                    "required": ["from", "to"]
                }
            },
            {
                "name": "get_blast_radius",
                "description": "Find all nodes affected if the given node changes. Returns affected nodes with risk level.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "node_id": { "type": "string", "description": "Node ID to analyze" }
                    },
                    "required": ["node_id"]
                }
            },
            {
                "name": "get_community",
                "description": "Get all nodes in a community cluster.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "community_id": { "type": "integer", "description": "Community ID number" }
                    },
                    "required": ["community_id"]
                }
            },
            {
                "name": "search_graph",
                "description": "Full-text search over node names and paths.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Search phrase" },
                        "limit": { "type": "integer", "description": "Max results (default 20)" }
                    },
                    "required": ["query"]
                }
            },
            {
                "name": "get_hotspots",
                "description": "Get highest-risk files by churn × coupling score.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "top_n": { "type": "integer", "description": "Number of results (default 10)" }
                    },
                    "required": []
                }
            },
            {
                "name": "get_file_owners",
                "description": "Get git blame ownership breakdown for a file.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "file_path": { "type": "string", "description": "Relative file path" }
                    },
                    "required": ["file_path"]
                }
            },
            {
                "name": "run_query",
                "description": "Run a read-only SQL query against the code graph database.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "sql": { "type": "string", "description": "SQL SELECT query to execute" }
                    },
                    "required": ["sql"]
                }
            }
        ]
    })
}
