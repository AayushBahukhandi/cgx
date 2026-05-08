use std::path::Path;

use cgx_engine::GraphDb;

/// Serialize to compact JSON (no indentation) to minimize token usage.
fn to_compact(v: &serde_json::Value) -> Result<String, String> {
    serde_json::to_string(v).map_err(|e| e.to_string())
}

pub fn handle_tool_call(
    name: &str,
    args: &serde_json::Value,
    repo_path: &Path,
) -> Result<String, String> {
    let db = || GraphDb::open(repo_path).map_err(|e| e.to_string());

    match name {
        "get_repo_summary" => tool_get_repo_summary(repo_path),
        "find_symbol" => {
            let name_val = get_str(args, "name")?;
            let kind = get_str_opt(args, "kind");
            tool_find_symbol(&db()?, &name_val, kind.as_deref())
        }
        "get_neighbors" => {
            let node_id = get_str(args, "node_id")?;
            let depth = get_u8(args, "depth", 1);
            tool_get_neighbors(&db()?, &node_id, depth)
        }
        "get_call_chain" => {
            let from = get_str(args, "from")?;
            let to = get_str(args, "to")?;
            tool_get_call_chain(&db()?, &from, &to)
        }
        "get_blast_radius" => {
            let node_id = get_str(args, "node_id")?;
            tool_get_blast_radius(&db()?, &node_id)
        }
        "get_community" => {
            let community_id = get_i64(args, "community_id")?;
            tool_get_community(&db()?, community_id)
        }
        "search_graph" => {
            let query = get_str(args, "query")?;
            let limit = get_u32(args, "limit", 20);
            tool_search_graph(&db()?, &query, limit)
        }
        "get_hotspots" => {
            let top_n = get_u64(args, "top_n", 10);
            tool_get_hotspots(&db()?, top_n as usize)
        }
        "get_file_owners" => {
            let file_path = get_str(args, "file_path")?;
            tool_get_file_owners(&db()?, &file_path)
        }
        "run_query" => {
            let sql = get_str(args, "sql")?;
            tool_run_query(&db()?, &sql)
        }
        _ => Err(format!("Unknown tool: {}", name)),
    }
}

fn tool_get_repo_summary(repo_path: &Path) -> Result<String, String> {
    let db = GraphDb::open(repo_path).map_err(|e| e.to_string())?;
    let node_count = db.node_count().map_err(|e| e.to_string())?;
    let edge_count = db.edge_count().map_err(|e| e.to_string())?;
    let languages = db.get_language_breakdown().map_err(|e| e.to_string())?;
    let communities = db.get_communities().map_err(|e| e.to_string())?;
    let hotspots = db.get_hotspots(5).map_err(|e| e.to_string())?;
    let all_nodes = db.get_all_nodes().map_err(|e| e.to_string())?;

    let mut sorted: Vec<&cgx_engine::Node> =
        all_nodes.iter().filter(|n| n.kind != "File").collect();
    sorted.sort_by_key(|b| std::cmp::Reverse(b.in_degree));
    let god_nodes: Vec<serde_json::Value> = sorted.iter().take(5)
        .map(|n| serde_json::json!({"id":n.id,"name":n.name,"kind":n.kind,"in_degree":n.in_degree}))
        .collect();

    // Cap to top 15 communities (sorted by node_count desc) — returning all wastes thousands of tokens
    let communities_json: Vec<serde_json::Value> = communities
        .iter()
        .take(15)
        .map(|(id, label, count, top_nodes)| {
            serde_json::json!({"id":id,"label":label,"node_count":count,
                "top_nodes":top_nodes.iter().take(3).collect::<Vec<_>>()})
        })
        .collect();

    let hotspots_json: Vec<serde_json::Value> = hotspots
        .iter()
        .map(|(path, churn, _coupling, callers)| {
            serde_json::json!({"path":path,"churn":churn,"callers":callers})
        })
        .collect();

    // Language summary string for the plain-text header
    let mut lang_entries: Vec<_> = languages.iter().collect();
    lang_entries.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
    let lang_str = lang_entries.iter()
        .map(|(l, p)| format!("{} {:.0}%", l, *p * 100.0))
        .collect::<Vec<_>>().join(", ");

    let top_hotspot = hotspots.first().map(|(p, ..)| p.as_str()).unwrap_or("none");
    let top_god = god_nodes.first()
        .and_then(|n| n.get("name")).and_then(|v| v.as_str()).unwrap_or("none");

    // Plain-text summary line — lets the model grasp the key facts before parsing JSON
    let summary = format!(
        "Repo: {} nodes, {} edges | {} | {} communities | hotspot: {} | most-used: {}",
        node_count, edge_count, lang_str, communities.len(), top_hotspot, top_god
    );

    let data = serde_json::json!({
        "_summary": summary,
        "node_count": node_count,
        "edge_count": edge_count,
        "community_count": communities.len(),
        "languages": languages,
        "top_communities": communities_json,
        "hotspots": hotspots_json,
        "god_nodes": god_nodes,
    });

    to_compact(&data)
}

fn tool_find_symbol(db: &GraphDb, name: &str, kind: Option<&str>) -> Result<String, String> {
    let all = db.get_all_nodes().map_err(|e| e.to_string())?;
    let query = name.to_lowercase();
    let results: Vec<_> = all
        .iter()
        .filter(|n| {
            if let Some(k) = kind {
                if n.kind != k { return false; }
            }
            n.name.to_lowercase().contains(&query) || n.id.to_lowercase().contains(&query)
        })
        .take(20)
        // Return only fields useful for navigation — omit line_end, churn, community (saves ~40% per node)
        .map(|n| serde_json::json!({
            "id":n.id,"kind":n.kind,"name":n.name,
            "path":n.path,"line":n.line_start,
            "in_degree":n.in_degree,
        }))
        .collect();
    let summary = format!("{} match(es) for '{}'", results.len(), name);
    to_compact(&serde_json::json!({"_summary":summary,"nodes":results}))
}

fn tool_get_neighbors(db: &GraphDb, node_id: &str, depth: u8) -> Result<String, String> {
    let neighbors = db
        .get_neighbors(node_id, depth.min(3))
        .map_err(|e| e.to_string())?;
    let all_edges = db.get_all_edges().map_err(|e| e.to_string())?;
    // Only include edges that connect TO or FROM the queried node — not edges between all
    // neighbor pairs (which causes O(n²) blowup on high-degree nodes).
    let edges: Vec<_> = all_edges.iter()
        .filter(|e| e.src == node_id || e.dst == node_id)
        .map(|e| serde_json::json!({"src":e.src,"dst":e.dst,"kind":e.kind}))
        .collect();
    let total = neighbors.len();
    let nodes: Vec<_> = neighbors.iter().take(50)
        .map(|n| serde_json::json!({"id":n.id,"kind":n.kind,"name":n.name,"path":n.path}))
        .collect();
    let summary = format!("{} neighbor(s) of '{}'{}", total, node_id,
        if total > 50 { " (showing first 50)" } else { "" });
    to_compact(&serde_json::json!({"_summary":summary,"nodes":nodes,"edges":edges,"truncated":total>50}))
}

fn tool_get_call_chain(db: &GraphDb, from: &str, to: &str) -> Result<String, String> {
    let all = db.get_all_nodes().map_err(|e| e.to_string())?;
    let node_map: std::collections::HashMap<&str, &cgx_engine::Node> =
        all.iter().map(|n| (n.id.as_str(), n)).collect();

    let from_id = resolve_node_id(&all, from);
    let to_id = resolve_node_id(&all, to);

    let all_edges = db.get_all_edges().map_err(|e| e.to_string())?;
    let mut adj: std::collections::HashMap<&str, Vec<&str>> = std::collections::HashMap::new();
    for e in &all_edges {
        adj.entry(e.src.as_str()).or_default().push(e.dst.as_str());
    }

    if let (Some(fid), Some(tid)) = (&from_id, &to_id) {
        let mut queue = std::collections::VecDeque::new();
        let mut visited = std::collections::HashSet::new();
        let mut parent: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
        queue.push_back(fid.as_str());
        visited.insert(fid.as_str());

        while let Some(current) = queue.pop_front() {
            if current == tid.as_str() {
                let mut path = vec![current];
                let mut cur = current;
                while let Some(&p) = parent.get(cur) {
                    path.push(p);
                    cur = p;
                }
                path.reverse();
                let path_json: Vec<_> = path
                    .iter()
                    .filter_map(|id| node_map.get(id))
                    .map(|n| serde_json::json!({ "id": n.id, "name": n.name, "kind": n.kind }))
                    .collect();
                // Build edges between consecutive nodes in the path
                let mut path_edges = Vec::new();
                for window in path.windows(2) {
                    let src = window[0];
                    let dst = window[1];
                    if let Some(edge) = all_edges.iter().find(|e| e.src == src && e.dst == dst) {
                        path_edges.push(serde_json::json!({
                            "src": edge.src, "dst": edge.dst, "kind": edge.kind,
                            "weight": edge.weight, "confidence": edge.confidence,
                        }));
                    }
                }
                let summary = format!("Path found: {} → {} ({} hops)", from, to, path_json.len() - 1);
                return to_compact(&serde_json::json!({
                    "_summary": summary, "found": true, "path": path_json, "edges": path_edges,
                }));
            }
            if let Some(nexts) = adj.get(current) {
                for &next in nexts {
                    if visited.insert(next) {
                        parent.insert(next, current);
                        queue.push_back(next);
                    }
                }
            }
        }
        to_compact(&serde_json::json!({"_summary":"No path found","found":false,"path":[],"edges":[]}))
    } else {
        to_compact(&serde_json::json!({"_summary":"Could not resolve one or both symbols","found":false,"path":[],"edges":[]}))
    }
}

fn tool_get_blast_radius(db: &GraphDb, node_id: &str) -> Result<String, String> {
    let all = db.get_all_nodes().map_err(|e| e.to_string())?;
    let resolved = resolve_node_id(&all, node_id).unwrap_or_else(|| node_id.to_string());
    let neighbors = db.get_dependents(&resolved, 3).map_err(|e| e.to_string())?;
    let affected_count = neighbors.len() as u32;
    let risk = if affected_count > 50 { "CRITICAL" }
               else if affected_count > 20 { "HIGH" }
               else if affected_count > 5 { "MEDIUM" }
               else { "LOW" };

    // Cap the node list to 30; for larger blast radii return a sample + breakdown by kind.
    // This avoids returning 500-node lists (each ~80 chars) for god nodes.
    let sample: Vec<_> = neighbors.iter().take(30)
        .map(|n| serde_json::json!({"id":n.id,"name":n.name,"kind":n.kind,"path":n.path}))
        .collect();

    let mut by_kind: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
    for n in &neighbors {
        *by_kind.entry(n.kind.as_str()).or_default() += 1;
    }

    let summary = format!("{} affected ({}) — changing '{}' breaks these", affected_count, risk, node_id);
    to_compact(&serde_json::json!({
        "_summary": summary,
        "affected_count": affected_count,
        "risk": risk,
        "by_kind": by_kind,
        "sample": sample,
        "truncated": affected_count > 30,
    }))
}

fn tool_get_community(db: &GraphDb, community_id: i64) -> Result<String, String> {
    let nodes = db
        .get_nodes_by_community(community_id)
        .map_err(|e| e.to_string())?;
    let edges = db
        .get_edges_by_community(community_id)
        .map_err(|e| e.to_string())?;
    let communities = db.get_communities().map_err(|e| e.to_string())?;
    let label = communities
        .iter()
        .find(|(id, ..)| *id == community_id)
        .map(|(_, label, _, _)| label.clone())
        .unwrap_or_else(|| format!("community-{}", community_id));
    let nodes_json: Vec<_> = nodes
        .iter()
        .map(|n| {
            serde_json::json!({
                "id": n.id, "kind": n.kind, "name": n.name, "path": n.path,
            })
        })
        .collect();
    let summary = format!("Community '{}': {} nodes, {} edges", label, nodes_json.len(), edges.len());
    to_compact(&serde_json::json!({"_summary":summary,"nodes":nodes_json,"label":label,"edge_count":edges.len()}))
}

fn tool_search_graph(db: &GraphDb, query: &str, limit: u32) -> Result<String, String> {
    let all = db.get_all_nodes().map_err(|e| e.to_string())?;
    let q = query.to_lowercase();
    let results: Vec<_> = all.iter()
        .filter(|n| n.name.to_lowercase().contains(&q) || n.path.to_lowercase().contains(&q))
        .take(limit as usize)
        .map(|n| serde_json::json!({"id":n.id,"kind":n.kind,"name":n.name,"path":n.path}))
        .collect();
    let summary = format!("{} result(s) for '{}'", results.len(), query);
    to_compact(&serde_json::json!({"_summary":summary,"nodes":results}))
}

fn tool_get_hotspots(db: &GraphDb, top_n: usize) -> Result<String, String> {
    let hotspots = db.get_hotspots(top_n).map_err(|e| e.to_string())?;
    let all_edges = db.get_all_edges().map_err(|e| e.to_string())?;
    let all_nodes = db.get_all_nodes().map_err(|e| e.to_string())?;
    let results: Vec<_> = hotspots
        .iter()
        .map(|(path, churn, coupling, callers)| {
            let file_node_id = format!("file:{}", path);
            let owner = all_edges
                .iter()
                .find(|e| e.kind == "OWNS" && e.dst == file_node_id)
                .and_then(|e| all_nodes.iter().find(|n| n.id == e.src))
                .map(|n| n.name.clone())
                .unwrap_or_default();
            serde_json::json!({
                "path": path, "churn": churn, "coupling": coupling,
                "caller_count": callers, "owner": owner,
            })
        })
        .collect();
    let top = results.first().and_then(|r| r.get("path")).and_then(|v| v.as_str()).unwrap_or("none");
    let summary = format!("{} hotspot(s); riskiest: {}", results.len(), top);
    to_compact(&serde_json::json!({"_summary":summary,"hotspots":results}))
}

fn tool_get_file_owners(db: &GraphDb, file_path: &str) -> Result<String, String> {
    let all_nodes = db.get_all_nodes().map_err(|e| e.to_string())?;
    let all_edges = db.get_all_edges().map_err(|e| e.to_string())?;
    let file_node_id = format!("file:{}", file_path);

    let total_lines = all_nodes
        .iter()
        .find(|n| n.id == file_node_id)
        .map(|n| n.line_end.saturating_sub(n.line_start) as f64)
        .unwrap_or(1.0)
        .max(1.0);

    let owners: Vec<_> = all_edges
        .iter()
        .filter(|e| e.kind == "OWNS" && e.dst == file_node_id)
        .filter_map(|e| {
            let author = all_nodes.iter().find(|n| n.id == e.src)?;
            let email = author
                .id
                .strip_prefix("author:")
                .unwrap_or(&author.id)
                .to_string();
            let lines = (e.weight * total_lines).round() as u32;
            Some(serde_json::json!({
                "name": author.name,
                "email": email,
                "pct": e.weight,
                "lines": lines,
            }))
        })
        .collect();
    let top_owner = owners.first().and_then(|o| o.get("name")).and_then(|v| v.as_str()).unwrap_or("unknown");
    let summary = format!("{} owner(s) for '{}'; primary: {}", owners.len(), file_path, top_owner);
    to_compact(&serde_json::json!({"_summary":summary,"owners":owners}))
}

fn tool_run_query(db: &GraphDb, sql: &str) -> Result<String, String> {
    let trimmed = sql.trim();
    // Must start with SELECT
    let first_word = trimmed
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_uppercase();
    if first_word != "SELECT" {
        return Err("Only SELECT queries are permitted".to_string());
    }
    // Block multi-statement injection via semicolons
    if trimmed.contains(';') {
        return Err("Only SELECT queries are permitted".to_string());
    }

    let mut stmt = db
        .conn
        .prepare(sql)
        .map_err(|e| format!("SQL error: {}", e))?;
    let col_count = stmt.column_count();
    let cols: Vec<String> = (0..col_count)
        .map(|i| {
            stmt.column_name(i)
                .map_or_else(|_| "unknown".to_string(), |s| s.to_string())
        })
        .collect();

    let duck_rows = stmt
        .query_map([], |row| {
            let mut vals = Vec::new();
            for i in 0..col_count {
                vals.push(match row.get::<_, String>(i) {
                    Ok(s) => serde_json::Value::String(s),
                    Err(_) => match row.get::<_, i64>(i) {
                        Ok(n) => serde_json::Value::Number(serde_json::Number::from(n)),
                        Err(_) => match row.get::<_, f64>(i) {
                            Ok(f) => serde_json::Number::from_f64(f)
                                .map(serde_json::Value::Number)
                                .unwrap_or(serde_json::Value::Null),
                            Err(_) => serde_json::Value::Null,
                        },
                    },
                });
            }
            Ok(vals)
        })
        .map_err(|e| format!("Query error: {}", e))?;

    let mut rows: Vec<serde_json::Value> = Vec::new();
    for row in duck_rows {
        let vals = row.map_err(|e| format!("Row error: {}", e))?;
        let map: serde_json::Map<String, serde_json::Value> =
            cols.iter().cloned().zip(vals).collect();
        rows.push(serde_json::Value::Object(map));
    }

    let summary = format!("{} row(s)", rows.len());
    to_compact(&serde_json::json!({"_summary":summary,"rows":rows,"columns":cols}))
}

fn get_str(args: &serde_json::Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("Missing required argument: {}", key))
}

fn get_str_opt(args: &serde_json::Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn get_u8(args: &serde_json::Value, key: &str, default: u8) -> u8 {
    args.get(key)
        .and_then(|v| v.as_u64())
        .map(|n| n.min(3) as u8)
        .unwrap_or(default)
}

fn get_u32(args: &serde_json::Value, key: &str, default: u32) -> u32 {
    args.get(key)
        .and_then(|v| v.as_u64())
        .map(|n| n as u32)
        .unwrap_or(default)
}

fn get_u64(args: &serde_json::Value, key: &str, default: u64) -> u64 {
    args.get(key).and_then(|v| v.as_u64()).unwrap_or(default)
}

fn get_i64(args: &serde_json::Value, key: &str) -> Result<i64, String> {
    args.get(key)
        .and_then(|v| v.as_i64())
        .ok_or_else(|| format!("Missing required argument: {}", key))
}

fn resolve_node_id(all_nodes: &[cgx_engine::Node], name_or_id: &str) -> Option<String> {
    if all_nodes.iter().any(|n| n.id == name_or_id) {
        return Some(name_or_id.to_string());
    }
    let query = name_or_id.to_lowercase();
    all_nodes
        .iter()
        .find(|n| n.name.to_lowercase() == query)
        .map(|n| n.id.clone())
        .or_else(|| {
            all_nodes
                .iter()
                .find(|n| n.name.to_lowercase().contains(&query))
                .map(|n| n.id.clone())
        })
}
