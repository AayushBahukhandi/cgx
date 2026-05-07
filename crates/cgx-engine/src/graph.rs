use std::path::{Path, PathBuf};

use duckdb::params;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::parser::{EdgeDef, NodeDef};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub path: String,
    pub line_start: u32,
    pub line_end: u32,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub churn: f64,
    #[serde(default)]
    pub coupling: f64,
    #[serde(default)]
    pub community: i64,
    #[serde(default)]
    pub in_degree: i64,
    #[serde(default)]
    pub out_degree: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub id: String,
    pub src: String,
    pub dst: String,
    pub kind: String,
    #[serde(default = "default_weight")]
    pub weight: f64,
    #[serde(default = "default_weight")]
    pub confidence: f64,
}

fn default_weight() -> f64 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoStats {
    pub node_count: u64,
    pub edge_count: u64,
    pub language_breakdown: std::collections::HashMap<String, f64>,
    pub community_count: u32,
    pub function_count: u64,
    pub class_count: u64,
    pub file_count: u64,
}

pub type CommunityRow = (i64, String, i64, Vec<String>);
type CommunityGroup = (Vec<(String, i64, String)>, i64); // (kind, in_degree, name)

impl Node {
    pub fn from_def(d: &NodeDef, language: &str) -> Self {
        Self {
            id: d.id.clone(),
            kind: d.kind.as_str().to_string(),
            name: d.name.clone(),
            path: d.path.clone(),
            line_start: d.line_start,
            line_end: d.line_end,
            language: language.to_string(),
            churn: 0.0,
            coupling: 0.0,
            community: 0,
            in_degree: 0,
            out_degree: 0,
        }
    }
}

impl Edge {
    pub fn from_def(d: &EdgeDef) -> Self {
        let id = format!("{}|{}|{}", d.src, d.kind.as_str(), d.dst);
        Self {
            id,
            src: d.src.clone(),
            dst: d.dst.clone(),
            kind: d.kind.as_str().to_string(),
            weight: d.weight,
            confidence: d.confidence,
        }
    }
}

pub struct GraphDb {
    pub conn: duckdb::Connection,
    pub repo_id: String,
    pub db_path: PathBuf,
}

impl GraphDb {
    pub fn open(repo_path: &Path) -> anyhow::Result<Self> {
        let repo_id = repo_hash(repo_path);
        let dir = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?
            .join(".cgx")
            .join("repos");
        std::fs::create_dir_all(&dir)?;

        let db_path = dir.join(format!("{}.db", repo_id));
        let conn = duckdb::Connection::open(&db_path)?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS nodes (
                id         VARCHAR PRIMARY KEY,
                kind       VARCHAR NOT NULL,
                name       VARCHAR NOT NULL,
                path       VARCHAR NOT NULL,
                line_start INTEGER,
                line_end   INTEGER,
                language   VARCHAR,
                churn      DOUBLE DEFAULT 0.0,
                coupling   DOUBLE DEFAULT 0.0,
                community  BIGINT DEFAULT 0,
                in_degree  BIGINT DEFAULT 0,
                out_degree BIGINT DEFAULT 0,
                metadata   JSON
            );
            CREATE TABLE IF NOT EXISTS edges (
                id         VARCHAR PRIMARY KEY,
                src        VARCHAR NOT NULL,
                dst        VARCHAR NOT NULL,
                kind       VARCHAR NOT NULL,
                weight     DOUBLE DEFAULT 1.0,
                confidence DOUBLE DEFAULT 1.0,
                metadata   JSON
            );
            CREATE TABLE IF NOT EXISTS communities (
                id         INTEGER PRIMARY KEY,
                label      VARCHAR,
                node_count INTEGER,
                top_nodes  JSON
            );
            CREATE TABLE IF NOT EXISTS repo_meta (
                key        VARCHAR PRIMARY KEY,
                value      JSON
            );
            CREATE TABLE IF NOT EXISTS file_hashes (
                path       VARCHAR PRIMARY KEY,
                hash       VARCHAR NOT NULL,
                indexed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );
            CREATE INDEX IF NOT EXISTS idx_nodes_kind      ON nodes(kind);
            CREATE INDEX IF NOT EXISTS idx_nodes_path      ON nodes(path);
            CREATE INDEX IF NOT EXISTS idx_nodes_community ON nodes(community);
            CREATE INDEX IF NOT EXISTS idx_edges_src       ON edges(src);
            CREATE INDEX IF NOT EXISTS idx_edges_dst       ON edges(dst);
            CREATE INDEX IF NOT EXISTS idx_edges_kind      ON edges(kind);",
        )?;

        Ok(Self {
            conn,
            repo_id,
            db_path,
        })
    }

    pub fn upsert_nodes(&self, nodes: &[Node]) -> anyhow::Result<usize> {
        if nodes.is_empty() {
            return Ok(0);
        }
        let mut count = 0;
        let mut stmt = self.conn.prepare(
            "INSERT OR REPLACE INTO nodes (id, kind, name, path, line_start, line_end, language, churn, coupling, community, in_degree, out_degree)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )?;
        for node in nodes {
            stmt.execute(params![
                node.id,
                node.kind,
                node.name,
                node.path,
                node.line_start,
                node.line_end,
                node.language,
                node.churn,
                node.coupling,
                node.community,
                node.in_degree,
                node.out_degree,
            ])?;
            count += 1;
        }
        Ok(count)
    }

    pub fn upsert_edges(&self, edges: &[Edge]) -> anyhow::Result<usize> {
        if edges.is_empty() {
            return Ok(0);
        }
        let mut count = 0;
        let mut stmt = self.conn.prepare(
            "INSERT OR REPLACE INTO edges (id, src, dst, kind, weight, confidence)
             VALUES (?, ?, ?, ?, ?, ?)",
        )?;
        for edge in edges {
            stmt.execute(params![
                edge.id,
                edge.src,
                edge.dst,
                edge.kind,
                edge.weight,
                edge.confidence,
            ])?;
            count += 1;
        }
        Ok(count)
    }

    pub fn get_node(&self, id: &str) -> anyhow::Result<Option<Node>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, kind, name, path, line_start, line_end, language, churn, coupling, community, in_degree, out_degree FROM nodes WHERE id = ?")?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok(Node {
                id: row.get(0)?,
                kind: row.get(1)?,
                name: row.get(2)?,
                path: row.get(3)?,
                line_start: row.get(4)?,
                line_end: row.get(5)?,
                language: row.get(6)?,
                churn: row.get(7)?,
                coupling: row.get(8)?,
                community: row.get(9)?,
                in_degree: row.get(10)?,
                out_degree: row.get(11)?,
            })
        })?;

        match rows.next() {
            Some(Ok(node)) => Ok(Some(node)),
            _ => Ok(None),
        }
    }

    pub fn get_neighbors(&self, id: &str, depth: u8) -> anyhow::Result<Vec<Node>> {
        let mut seen = std::collections::HashSet::new();
        seen.insert(id.to_string());
        let mut current = vec![id.to_string()];
        let mut result: Vec<Node> = Vec::new();
        let max_depth = depth.min(3);

        for _ in 0..max_depth {
            if current.is_empty() {
                break;
            }
            let mut next = Vec::new();

            for cur_id in &current {
                let mut stmt = self.conn.prepare(
                    "SELECT DISTINCT n.id, n.kind, n.name, n.path, n.line_start, n.line_end, n.language, n.churn, n.coupling, n.community, n.in_degree, n.out_degree
                     FROM nodes n
                     INNER JOIN edges e ON (e.dst = n.id AND e.src = ?1) OR (e.src = n.id AND e.dst = ?2)
                     LIMIT 100",
                )?;
                let rows = stmt.query_map(params![cur_id, cur_id], |row| {
                    Ok(Node {
                        id: row.get(0)?,
                        kind: row.get(1)?,
                        name: row.get(2)?,
                        path: row.get(3)?,
                        line_start: row.get(4)?,
                        line_end: row.get(5)?,
                        language: row.get(6)?,
                        churn: row.get(7)?,
                        coupling: row.get(8)?,
                        community: row.get(9)?,
                        in_degree: row.get(10)?,
                        out_degree: row.get(11)?,
                    })
                })?;

                for row in rows {
                    let node = row?;
                    if seen.insert(node.id.clone()) {
                        next.push(node.id.clone());
                        result.push(node);
                    }
                }
            }
            current = next;
        }

        Ok(result)
    }

    pub fn get_all_nodes(&self) -> anyhow::Result<Vec<Node>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, name, path, line_start, line_end, language, churn, coupling, community, in_degree, out_degree FROM nodes",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Node {
                id: row.get(0)?,
                kind: row.get(1)?,
                name: row.get(2)?,
                path: row.get(3)?,
                line_start: row.get(4)?,
                line_end: row.get(5)?,
                language: row.get(6)?,
                churn: row.get(7)?,
                coupling: row.get(8)?,
                community: row.get(9)?,
                in_degree: row.get(10)?,
                out_degree: row.get(11)?,
            })
        })?;

        let mut nodes = Vec::new();
        for row in rows {
            nodes.push(row?);
        }
        Ok(nodes)
    }

    pub fn get_all_edges(&self) -> anyhow::Result<Vec<Edge>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, src, dst, kind, weight, confidence FROM edges",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Edge {
                id: row.get(0)?,
                src: row.get(1)?,
                dst: row.get(2)?,
                kind: row.get(3)?,
                weight: row.get(4)?,
                confidence: row.get(5)?,
            })
        })?;

        let mut edges = Vec::new();
        for row in rows {
            edges.push(row?);
        }
        Ok(edges)
    }

    pub fn node_count(&self) -> anyhow::Result<u64> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM nodes", [], |row| row.get(0))?;
        Ok(count as u64)
    }

    pub fn edge_count(&self) -> anyhow::Result<u64> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM edges", [], |row| row.get(0))?;
        Ok(count as u64)
    }

    pub fn clear(&self) -> anyhow::Result<()> {
        // Drop and recreate tables instead of DELETE to avoid DuckDB ART index
        // bulk-delete failures on large datasets (duckdb issue with indexed tables).
        self.conn.execute_batch(
            "DROP TABLE IF EXISTS edges;
             DROP TABLE IF EXISTS nodes;
             DROP TABLE IF EXISTS communities;
             CREATE TABLE IF NOT EXISTS nodes (
                 id         VARCHAR PRIMARY KEY,
                 kind       VARCHAR NOT NULL,
                 name       VARCHAR NOT NULL,
                 path       VARCHAR NOT NULL,
                 line_start INTEGER,
                 line_end   INTEGER,
                 language   VARCHAR,
                 churn      DOUBLE DEFAULT 0.0,
                 coupling   DOUBLE DEFAULT 0.0,
                 community  BIGINT DEFAULT 0,
                 in_degree  BIGINT DEFAULT 0,
                 out_degree BIGINT DEFAULT 0,
                 metadata   JSON
             );
             CREATE TABLE IF NOT EXISTS edges (
                 id         VARCHAR PRIMARY KEY,
                 src        VARCHAR NOT NULL,
                 dst        VARCHAR NOT NULL,
                 kind       VARCHAR NOT NULL,
                 weight     DOUBLE DEFAULT 1.0,
                 confidence DOUBLE DEFAULT 1.0,
                 metadata   JSON
             );
             CREATE TABLE IF NOT EXISTS communities (
                 id         INTEGER PRIMARY KEY,
                 label      VARCHAR,
                 node_count INTEGER,
                 top_nodes  JSON
             );
             CREATE INDEX IF NOT EXISTS idx_nodes_kind      ON nodes(kind);
             CREATE INDEX IF NOT EXISTS idx_nodes_path      ON nodes(path);
             CREATE INDEX IF NOT EXISTS idx_nodes_community ON nodes(community);
             CREATE INDEX IF NOT EXISTS idx_edges_src       ON edges(src);
             CREATE INDEX IF NOT EXISTS idx_edges_dst       ON edges(dst);
             CREATE INDEX IF NOT EXISTS idx_edges_kind      ON edges(kind);",
        )?;
        Ok(())
    }

    pub fn get_language_breakdown(&self) -> anyhow::Result<std::collections::HashMap<String, f64>> {
        let mut stmt = self.conn.prepare(
            "SELECT language, COUNT(*) as cnt FROM nodes WHERE language != '' GROUP BY language",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;

        let mut counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        for row in rows {
            let (lang, cnt) = row?;
            *counts.entry(lang).or_default() += cnt;
        }

        let total: i64 = counts.values().sum();
        if total == 0 {
            return Ok(std::collections::HashMap::new());
        }

        let mut breakdown = std::collections::HashMap::new();
        for (lang, cnt) in counts {
            breakdown.insert(lang, cnt as f64 / total as f64);
        }
        Ok(breakdown)
    }

    pub fn get_node_counts_by_kind(&self) -> anyhow::Result<std::collections::HashMap<String, u64>> {
        let mut stmt = self.conn.prepare(
            "SELECT kind, COUNT(*) as cnt FROM nodes GROUP BY kind",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;

        let mut counts = std::collections::HashMap::new();
        for row in rows {
            let (kind, cnt) = row?;
            counts.insert(kind, cnt as u64);
        }
        Ok(counts)
    }

    pub fn upsert_node_scores(&self, node_id: &str, churn: f64, coupling: f64) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE nodes SET churn = ?, coupling = ? WHERE id = ?",
            params![churn, coupling, node_id],
        )?;
        Ok(())
    }

    pub fn update_in_out_degrees(&self) -> anyhow::Result<()> {
        self.conn.execute_batch(
            "UPDATE nodes SET in_degree = 0, out_degree = 0;
             UPDATE nodes SET out_degree = (SELECT COUNT(*) FROM edges WHERE edges.src = nodes.id);
             UPDATE nodes SET in_degree = (SELECT COUNT(*) FROM edges WHERE edges.dst = nodes.id);",
        )?;
        Ok(())
    }

    pub fn get_hotspots(&self, limit: usize) -> anyhow::Result<Vec<(String, f64, f64, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT path, churn, coupling, in_degree
             FROM nodes
             WHERE kind = 'File' AND (churn > 0.0 OR in_degree > 0)
             ORDER BY (churn * COALESCE(coupling, 0.0) + CAST(in_degree AS DOUBLE) * 0.01) DESC
             LIMIT ?",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, f64>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    pub fn get_ownership(&self) -> anyhow::Result<Vec<(String, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT n.name, COUNT(e.id) as file_count
             FROM nodes n
             INNER JOIN edges e ON e.src = n.id AND e.kind = 'OWNS'
             WHERE n.kind = 'Author'
             GROUP BY n.name
             ORDER BY file_count DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    pub fn compute_coupling(&self) -> anyhow::Result<()> {
        self.conn.execute_batch(
            "UPDATE nodes SET coupling = 0.0;
             UPDATE nodes SET coupling = 
                CASE 
                    WHEN (SELECT MAX(in_degree) FROM nodes WHERE kind = 'File') > 0
                    THEN CAST(in_degree AS DOUBLE) / CAST((SELECT MAX(in_degree) FROM nodes WHERE kind = 'File') AS DOUBLE)
                    ELSE 0.0
                END
             WHERE kind = 'File';",
        )?;
        Ok(())
    }

    pub fn update_node_communities(&self, communities: &std::collections::HashMap<String, i64>) -> anyhow::Result<usize> {
        if communities.is_empty() {
            return Ok(0);
        }
        let mut count = 0;
        let mut stmt = self.conn.prepare("UPDATE nodes SET community = ? WHERE id = ?")?;
        for (node_id, community) in communities {
            let affected = stmt.execute(params![*community, node_id.as_str()])?;
            count += affected;
        }
        Ok(count)
    }

    pub fn get_stats(&self) -> anyhow::Result<RepoStats> {
        let node_count = self.node_count()?;
        let edge_count = self.edge_count()?;
        let lang_breakdown = self.get_language_breakdown()?;
        let communities = self.get_communities()?;
        let counts_by_kind = self.get_node_counts_by_kind()?;

        Ok(RepoStats {
            node_count,
            edge_count,
            language_breakdown: lang_breakdown,
            community_count: communities.len() as u32,
            function_count: counts_by_kind.get("Function").copied().unwrap_or(0),
            class_count: counts_by_kind.get("Class").copied().unwrap_or(0),
            file_count: counts_by_kind.get("File").copied().unwrap_or(0),
        })
    }

    pub fn get_entry_points(&self, limit: usize) -> anyhow::Result<Vec<Node>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, name, path, line_start, line_end, language, churn, coupling, community, in_degree, out_degree
             FROM nodes
             WHERE in_degree = 0 AND kind != 'File' AND kind != 'Author'
             ORDER BY out_degree DESC
             LIMIT ?",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(Node {
                id: row.get(0)?,
                kind: row.get(1)?,
                name: row.get(2)?,
                path: row.get(3)?,
                line_start: row.get(4)?,
                line_end: row.get(5)?,
                language: row.get(6)?,
                churn: row.get(7)?,
                coupling: row.get(8)?,
                community: row.get(9)?,
                in_degree: row.get(10)?,
                out_degree: row.get(11)?,
            })
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    pub fn get_god_nodes(&self, limit: usize) -> anyhow::Result<Vec<Node>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, name, path, line_start, line_end, language, churn, coupling, community, in_degree, out_degree
             FROM nodes
             WHERE in_degree > 0 AND kind != 'File' AND kind != 'Author'
             ORDER BY in_degree DESC
             LIMIT ?",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(Node {
                id: row.get(0)?,
                kind: row.get(1)?,
                name: row.get(2)?,
                path: row.get(3)?,
                line_start: row.get(4)?,
                line_end: row.get(5)?,
                language: row.get(6)?,
                churn: row.get(7)?,
                coupling: row.get(8)?,
                community: row.get(9)?,
                in_degree: row.get(10)?,
                out_degree: row.get(11)?,
            })
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    pub fn get_communities(&self) -> anyhow::Result<Vec<CommunityRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT community, kind, name, path, in_degree
             FROM nodes
             WHERE community > 0
             ORDER BY community",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })?;

        let mut community_map: std::collections::HashMap<i64, CommunityGroup> = std::collections::HashMap::new();
        for row in rows {
            let (community, kind, name, _path, in_degree) = row?;
            let entry = community_map.entry(community).or_insert_with(|| (Vec::new(), 0));
            entry.0.push((kind, in_degree, name));
            entry.1 += 1;
        }

        let mut result: Vec<CommunityRow> = community_map
            .into_iter()
            .map(|(community, (mut items, count))| {
                items.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.2.cmp(&b.2)));
                let top_nodes: Vec<String> = items
                    .iter()
                    .take(5)
                    .map(|(kind, _deg, name)| format!("{}:{}", kind, name))
                    .collect();
                let label = top_nodes.first().cloned().unwrap_or_else(|| format!("community-{}", community));
                (community, label, count, top_nodes)
            })
            .collect();

        result.sort_by_key(|row| std::cmp::Reverse(row.2));
        Ok(result)
    }

    pub fn clear_communities(&self) -> anyhow::Result<()> {
        self.conn.execute("UPDATE nodes SET community = 0", [])?;
        self.conn.execute("DELETE FROM communities", [])?;
        Ok(())
    }

    /// BFS following only incoming edges — returns all nodes that depend on `id`.
    /// Used for blast-radius analysis: if `id` changes, these nodes are affected.
    pub fn get_dependents(&self, id: &str, depth: u8) -> anyhow::Result<Vec<Node>> {
        let mut seen = std::collections::HashSet::new();
        seen.insert(id.to_string());
        let mut current = vec![id.to_string()];
        let mut result: Vec<Node> = Vec::new();
        let max_depth = depth.min(3);

        for _ in 0..max_depth {
            if current.is_empty() {
                break;
            }
            let mut next = Vec::new();
            for cur_id in &current {
                let mut stmt = self.conn.prepare(
                    "SELECT DISTINCT n.id, n.kind, n.name, n.path, n.line_start, n.line_end, n.language, n.churn, n.coupling, n.community, n.in_degree, n.out_degree
                     FROM nodes n
                     INNER JOIN edges e ON e.src = n.id AND e.dst = ?
                     LIMIT 100",
                )?;
                let rows = stmt.query_map(params![cur_id], |row| {
                    Ok(Node {
                        id: row.get(0)?,
                        kind: row.get(1)?,
                        name: row.get(2)?,
                        path: row.get(3)?,
                        line_start: row.get(4)?,
                        line_end: row.get(5)?,
                        language: row.get(6)?,
                        churn: row.get(7)?,
                        coupling: row.get(8)?,
                        community: row.get(9)?,
                        in_degree: row.get(10)?,
                        out_degree: row.get(11)?,
                    })
                })?;
                for row in rows {
                    let node = row?;
                    if seen.insert(node.id.clone()) {
                        next.push(node.id.clone());
                        result.push(node);
                    }
                }
            }
            current = next;
        }

        Ok(result)
    }

    pub fn get_nodes_by_community(&self, community: i64) -> anyhow::Result<Vec<Node>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, name, path, line_start, line_end, language, churn, coupling, community, in_degree, out_degree FROM nodes WHERE community = ?",
        )?;
        let rows = stmt.query_map(params![community], |row| {
            Ok(Node {
                id: row.get(0)?,
                kind: row.get(1)?,
                name: row.get(2)?,
                path: row.get(3)?,
                line_start: row.get(4)?,
                line_end: row.get(5)?,
                language: row.get(6)?,
                churn: row.get(7)?,
                coupling: row.get(8)?,
                community: row.get(9)?,
                in_degree: row.get(10)?,
                out_degree: row.get(11)?,
            })
        })?;
        let mut nodes = Vec::new();
        for row in rows {
            nodes.push(row?);
        }
        Ok(nodes)
    }

    pub fn get_edges_by_community(&self, community: i64) -> anyhow::Result<Vec<Edge>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT e.id, e.src, e.dst, e.kind, e.weight, e.confidence
             FROM edges e
             INNER JOIN nodes n1 ON e.src = n1.id AND n1.community = ?
             INNER JOIN nodes n2 ON e.dst = n2.id AND n2.community = ?",
        )?;
        let rows = stmt.query_map(params![community, community], |row| {
            Ok(Edge {
                id: row.get(0)?,
                src: row.get(1)?,
                dst: row.get(2)?,
                kind: row.get(3)?,
                weight: row.get(4)?,
                confidence: row.get(5)?,
            })
        })?;
        let mut edges = Vec::new();
        for row in rows {
            edges.push(row?);
        }
        Ok(edges)
    }

    // ── File hashes for incremental indexing ────────────────────────────────

    pub fn get_file_hashes(&self) -> anyhow::Result<std::collections::HashMap<String, String>> {
        let mut stmt = self.conn.prepare("SELECT path, hash FROM file_hashes")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut result = std::collections::HashMap::new();
        for row in rows {
            let (path, hash) = row?;
            result.insert(path, hash);
        }
        Ok(result)
    }

    pub fn set_file_hash(&self, path: &str, hash: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO file_hashes (path, hash) VALUES (?, ?)",
            params![path, hash],
        )?;
        Ok(())
    }

    pub fn remove_file_hashes(&self, paths: &[String]) -> anyhow::Result<()> {
        if paths.is_empty() {
            return Ok(());
        }
        let placeholders = paths.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!("DELETE FROM file_hashes WHERE path IN ({})", placeholders);
        let mut stmt = self.conn.prepare(&sql)?;
        let params: Vec<&dyn duckdb::ToSql> = paths.iter().map(|p| p as &dyn duckdb::ToSql).collect();
        stmt.execute(params.as_slice())?;
        Ok(())
    }

    pub fn delete_nodes_by_paths(&self, paths: &[String]) -> anyhow::Result<usize> {
        if paths.is_empty() {
            return Ok(0);
        }
        let placeholders = paths.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        // Delete edges connected to nodes from these paths first
        let sql_edges = format!(
            "DELETE FROM edges WHERE src IN (SELECT id FROM nodes WHERE path IN ({})) OR dst IN (SELECT id FROM nodes WHERE path IN ({}))",
            placeholders, placeholders
        );
        let mut stmt_edges = self.conn.prepare(&sql_edges)?;
        let params_edges: Vec<&dyn duckdb::ToSql> = paths.iter().chain(paths.iter()).map(|p| p as &dyn duckdb::ToSql).collect();
        stmt_edges.execute(params_edges.as_slice())?;

        // Delete nodes
        let sql_nodes = format!("DELETE FROM nodes WHERE path IN ({})", placeholders);
        let mut stmt_nodes = self.conn.prepare(&sql_nodes)?;
        let params_nodes: Vec<&dyn duckdb::ToSql> = paths.iter().map(|p| p as &dyn duckdb::ToSql).collect();
        let count = stmt_nodes.execute(params_nodes.as_slice())?;
        Ok(count)
    }
}

pub fn repo_hash(path: &Path) -> String {
    let canonical = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf());
    let path_str = canonical.to_string_lossy().to_string();
    let mut hasher = Sha256::new();
    hasher.update(path_str.as_bytes());
    format!("{:x}", hasher.finalize())[..16].to_string()
}
