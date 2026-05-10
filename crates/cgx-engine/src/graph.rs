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
    #[serde(default)]
    pub exported: bool,
    #[serde(default)]
    pub is_dead_candidate: bool,
    #[serde(default)]
    pub dead_reason: Option<String>,
    #[serde(default)]
    pub complexity: f64,
    #[serde(default)]
    pub is_test_file: bool,
    #[serde(default)]
    pub test_count: i64,
    #[serde(default)]
    pub is_tested: bool,
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
/// (overall_pct, Vec<(community_id, documented, total)>, undocumented_high_coupling)
pub type DocsCoverage = (f64, Vec<(i64, i64, i64)>, Vec<Node>);
/// (overall_pct, tested_count, untested_count, gaps)
pub type TestCoverageSummary = (f64, i64, i64, Vec<Node>);
type CommunityGroup = (Vec<(String, i64, String)>, i64); // (kind, in_degree, name)

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotEntry {
    pub id: String,
    pub commit_sha: String,
    pub commit_date: String,
    pub commit_msg: String,
    pub node_count: i64,
    pub edge_count: i64,
    /// JSON blob: {"file_count": N, "insertions": N, "deletions": N}
    pub snapshot_data: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagRow {
    pub id: String,
    pub file_path: String,
    pub line: u32,
    pub tag_type: String,
    pub text: String,
    /// "code", "jsx", or "jsx_commented_code"
    pub comment_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloneRow {
    pub id: String,
    pub node_a: String,
    pub node_b: String,
    pub similarity: f64,
    pub kind: String,
}

impl Default for Node {
    fn default() -> Self {
        Self {
            id: String::new(),
            kind: String::new(),
            name: String::new(),
            path: String::new(),
            line_start: 0,
            line_end: 0,
            language: String::new(),
            churn: 0.0,
            coupling: 0.0,
            community: 0,
            in_degree: 0,
            out_degree: 0,
            exported: false,
            is_dead_candidate: false,
            dead_reason: None,
            complexity: 0.0,
            is_test_file: false,
            test_count: 0,
            is_tested: false,
        }
    }
}

impl Node {
    pub fn from_def(d: &NodeDef, language: &str) -> Self {
        let exported = d
            .metadata
            .get("exported")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let complexity = d
            .metadata
            .get("complexity")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
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
            exported,
            is_dead_candidate: false,
            dead_reason: None,
            complexity,
            is_test_file: false,
            test_count: 0,
            is_tested: false,
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
                id                 VARCHAR PRIMARY KEY,
                kind               VARCHAR NOT NULL,
                name               VARCHAR NOT NULL,
                path               VARCHAR NOT NULL,
                line_start         INTEGER,
                line_end           INTEGER,
                language           VARCHAR,
                churn              DOUBLE DEFAULT 0.0,
                coupling           DOUBLE DEFAULT 0.0,
                community          BIGINT DEFAULT 0,
                in_degree          BIGINT DEFAULT 0,
                out_degree         BIGINT DEFAULT 0,
                exported           TINYINT DEFAULT 0,
                is_dead_candidate  TINYINT DEFAULT 0,
                dead_reason        TEXT,
                metadata           JSON
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
            CREATE TABLE IF NOT EXISTS tags (
                id           VARCHAR PRIMARY KEY,
                file_path    VARCHAR NOT NULL,
                line         INTEGER NOT NULL,
                tag_type     VARCHAR NOT NULL,
                text         VARCHAR NOT NULL,
                comment_type VARCHAR NOT NULL DEFAULT 'code'
            );
            CREATE TABLE IF NOT EXISTS clones (
                id         VARCHAR PRIMARY KEY,
                node_a     VARCHAR NOT NULL,
                node_b     VARCHAR NOT NULL,
                similarity FLOAT NOT NULL,
                kind       VARCHAR NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_nodes_kind      ON nodes(kind);
            CREATE INDEX IF NOT EXISTS idx_nodes_path      ON nodes(path);
            CREATE INDEX IF NOT EXISTS idx_nodes_community ON nodes(community);
            CREATE INDEX IF NOT EXISTS idx_edges_src       ON edges(src);
            CREATE INDEX IF NOT EXISTS idx_edges_dst       ON edges(dst);
            CREATE INDEX IF NOT EXISTS idx_edges_kind      ON edges(kind);
            CREATE INDEX IF NOT EXISTS idx_tags_file       ON tags(file_path);
            CREATE INDEX IF NOT EXISTS idx_tags_type       ON tags(tag_type);
            CREATE INDEX IF NOT EXISTS idx_clones_a        ON clones(node_a);
            CREATE INDEX IF NOT EXISTS idx_clones_b        ON clones(node_b);",
        )?;

        // Migration: add new columns to existing DBs that pre-date this schema.
        // DuckDB 1.x supports "ADD COLUMN IF NOT EXISTS" which is a no-op when
        // the column is already present — no error, no transaction abort.
        conn.execute_batch(
            "ALTER TABLE nodes ADD COLUMN IF NOT EXISTS exported           TINYINT DEFAULT 0;
             ALTER TABLE nodes ADD COLUMN IF NOT EXISTS is_dead_candidate  TINYINT DEFAULT 0;
             ALTER TABLE nodes ADD COLUMN IF NOT EXISTS dead_reason        TEXT;
             ALTER TABLE nodes ADD COLUMN IF NOT EXISTS complexity         DOUBLE DEFAULT 0.0;
             ALTER TABLE nodes ADD COLUMN IF NOT EXISTS doc_comment        TEXT;
             ALTER TABLE nodes ADD COLUMN IF NOT EXISTS is_test_file       TINYINT DEFAULT 0;
             ALTER TABLE nodes ADD COLUMN IF NOT EXISTS test_count         INTEGER DEFAULT 0;
             ALTER TABLE nodes ADD COLUMN IF NOT EXISTS is_tested          TINYINT DEFAULT 0;
             CREATE INDEX IF NOT EXISTS idx_nodes_dead       ON nodes(is_dead_candidate);
             CREATE INDEX IF NOT EXISTS idx_nodes_complexity ON nodes(complexity);
             CREATE INDEX IF NOT EXISTS idx_nodes_is_tested  ON nodes(is_tested);",
        )?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS snapshots (
                id           VARCHAR PRIMARY KEY,
                commit_sha   VARCHAR NOT NULL,
                commit_date  TEXT NOT NULL,
                commit_msg   VARCHAR,
                node_count   INTEGER,
                edge_count   INTEGER,
                snapshot_data TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_snapshots_date ON snapshots(commit_date);",
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
            "INSERT OR REPLACE INTO nodes (id, kind, name, path, line_start, line_end, language, churn, coupling, community, in_degree, out_degree, exported, complexity, is_test_file, test_count, is_tested)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
                node.exported as i32,
                node.complexity,
                node.is_test_file as i32,
                node.test_count,
                node.is_tested as i32,
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

    pub fn upsert_tags(&self, tags: &[TagRow]) -> anyhow::Result<usize> {
        if tags.is_empty() {
            return Ok(0);
        }
        let mut count = 0;
        let mut stmt = self.conn.prepare(
            "INSERT OR REPLACE INTO tags (id, file_path, line, tag_type, text, comment_type)
             VALUES (?, ?, ?, ?, ?, ?)",
        )?;
        for tag in tags {
            stmt.execute(params![
                tag.id,
                tag.file_path,
                tag.line,
                tag.tag_type,
                tag.text,
                tag.comment_type,
            ])?;
            count += 1;
        }
        Ok(count)
    }

    pub fn get_tags(
        &self,
        tag_type_filter: Option<&str>,
        comment_type_filter: Option<&str>,
    ) -> anyhow::Result<Vec<TagRow>> {
        let sql = match (tag_type_filter, comment_type_filter) {
            (Some(_), Some(_)) => {
                "SELECT id, file_path, line, tag_type, text, comment_type FROM tags \
                 WHERE tag_type = ? AND comment_type = ? ORDER BY file_path, line"
            }
            (Some(_), None) => {
                "SELECT id, file_path, line, tag_type, text, comment_type FROM tags \
                 WHERE tag_type = ? ORDER BY file_path, line"
            }
            (None, Some(_)) => {
                "SELECT id, file_path, line, tag_type, text, comment_type FROM tags \
                 WHERE comment_type = ? ORDER BY file_path, line"
            }
            (None, None) => {
                "SELECT id, file_path, line, tag_type, text, comment_type FROM tags \
                 ORDER BY file_path, line"
            }
        };

        let mut stmt = self.conn.prepare(sql)?;
        let map_row = |row: &duckdb::Row| {
            Ok(TagRow {
                id: row.get(0)?,
                file_path: row.get(1)?,
                line: row.get::<_, u32>(2)?,
                tag_type: row.get(3)?,
                text: row.get(4)?,
                comment_type: row.get(5)?,
            })
        };

        let rows = match (tag_type_filter, comment_type_filter) {
            (Some(t), Some(c)) => stmt.query_map(params![t, c], map_row)?,
            (Some(t), None) => stmt.query_map(params![t], map_row)?,
            (None, Some(c)) => stmt.query_map(params![c], map_row)?,
            (None, None) => stmt.query_map([], map_row)?,
        };

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    pub fn clear_all_tags(&self) -> anyhow::Result<()> {
        self.conn.execute_batch(
            "DROP TABLE IF EXISTS tags;
             CREATE TABLE IF NOT EXISTS tags (
                 id           VARCHAR PRIMARY KEY,
                 file_path    VARCHAR NOT NULL,
                 line         INTEGER NOT NULL,
                 tag_type     VARCHAR NOT NULL,
                 text         VARCHAR NOT NULL,
                 comment_type VARCHAR NOT NULL DEFAULT 'code'
             );
             CREATE INDEX IF NOT EXISTS idx_tags_file ON tags(file_path);
             CREATE INDEX IF NOT EXISTS idx_tags_type ON tags(tag_type);",
        )?;
        Ok(())
    }

    pub fn delete_tags_for_paths(&self, paths: &[String]) -> anyhow::Result<()> {
        if paths.is_empty() {
            return Ok(());
        }
        let mut stmt = self.conn.prepare("DELETE FROM tags WHERE file_path = ?")?;
        for path in paths {
            stmt.execute(params![path])?;
        }
        Ok(())
    }

    pub fn get_node(&self, id: &str) -> anyhow::Result<Option<Node>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, kind, name, path, line_start, line_end, language, churn, coupling, community, in_degree, out_degree, COALESCE(exported, false) as exported, COALESCE(is_dead_candidate, false) as is_dead_candidate, dead_reason, COALESCE(complexity, 0.0), COALESCE(is_test_file, 0), COALESCE(test_count, 0), COALESCE(is_tested, 0) FROM nodes WHERE id = ?")?;
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
                exported: row.get::<_, i64>(12).map(|v| v != 0).unwrap_or(false),
                is_dead_candidate: row.get::<_, bool>(13).unwrap_or(false),
                dead_reason: row.get::<_, Option<String>>(14).unwrap_or(None),
                complexity: row.get::<_, f64>(15).unwrap_or(0.0),
                is_test_file: row.get::<_, i64>(16).map(|v| v != 0).unwrap_or(false),
                test_count: row.get::<_, i64>(17).unwrap_or(0),
                is_tested: row.get::<_, i64>(18).map(|v| v != 0).unwrap_or(false),
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
                    "SELECT DISTINCT n.id, n.kind, n.name, n.path, n.line_start, n.line_end, n.language, n.churn, n.coupling, n.community, n.in_degree, n.out_degree, COALESCE(n.exported, false), COALESCE(n.is_dead_candidate, false), n.dead_reason, COALESCE(n.complexity, 0.0), COALESCE(n.is_test_file, 0), COALESCE(n.test_count, 0), COALESCE(n.is_tested, 0)
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
                        exported: row.get::<_, i64>(12).map(|v| v != 0).unwrap_or(false),
                        is_dead_candidate: row.get::<_, bool>(13).unwrap_or(false),
                        dead_reason: row.get::<_, Option<String>>(14).unwrap_or(None),
                        complexity: row.get::<_, f64>(15).unwrap_or(0.0),
                        is_test_file: row.get::<_, i64>(16).map(|v| v != 0).unwrap_or(false),
                        test_count: row.get::<_, i64>(17).unwrap_or(0),
                        is_tested: row.get::<_, i64>(18).map(|v| v != 0).unwrap_or(false),
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
            "SELECT id, kind, name, path, line_start, line_end, language, churn, coupling, community, in_degree, out_degree, COALESCE(exported, false), COALESCE(is_dead_candidate, false), dead_reason, COALESCE(complexity, 0.0), COALESCE(is_test_file, 0), COALESCE(test_count, 0), COALESCE(is_tested, 0) FROM nodes",
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
                exported: row.get::<_, i64>(12).map(|v| v != 0).unwrap_or(false),
                is_dead_candidate: row.get::<_, bool>(13).unwrap_or(false),
                dead_reason: row.get::<_, Option<String>>(14).unwrap_or(None),
                complexity: row.get::<_, f64>(15).unwrap_or(0.0),
                is_test_file: row.get::<_, i64>(16).map(|v| v != 0).unwrap_or(false),
                test_count: row.get::<_, i64>(17).unwrap_or(0),
                is_tested: row.get::<_, i64>(18).map(|v| v != 0).unwrap_or(false),
            })
        })?;

        let mut nodes = Vec::new();
        for row in rows {
            nodes.push(row?);
        }
        Ok(nodes)
    }

    pub fn get_all_edges(&self) -> anyhow::Result<Vec<Edge>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, src, dst, kind, weight, confidence FROM edges")?;
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
        // TRUNCATE avoids DuckDB ART index bulk-delete failures on large datasets
        // and is more reliable than DROP+CREATE for data persistence across connections.
        self.conn.execute_batch(
            "TRUNCATE TABLE edges;
             TRUNCATE TABLE nodes;
             TRUNCATE TABLE communities;",
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

    pub fn get_node_counts_by_kind(
        &self,
    ) -> anyhow::Result<std::collections::HashMap<String, u64>> {
        let mut stmt = self
            .conn
            .prepare("SELECT kind, COUNT(*) as cnt FROM nodes GROUP BY kind")?;
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

    pub fn upsert_node_scores(
        &self,
        node_id: &str,
        churn: f64,
        coupling: f64,
    ) -> anyhow::Result<()> {
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

    pub fn update_node_communities(
        &self,
        communities: &std::collections::HashMap<String, i64>,
    ) -> anyhow::Result<usize> {
        if communities.is_empty() {
            return Ok(0);
        }
        let mut count = 0;
        let mut stmt = self
            .conn
            .prepare("UPDATE nodes SET community = ? WHERE id = ?")?;
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
            "SELECT id, kind, name, path, line_start, line_end, language, churn, coupling, community, in_degree, out_degree, COALESCE(exported, false), COALESCE(is_dead_candidate, false), dead_reason, COALESCE(complexity, 0.0), COALESCE(is_test_file, 0), COALESCE(test_count, 0), COALESCE(is_tested, 0)
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
                exported: row.get::<_, i64>(12).map(|v| v != 0).unwrap_or(false),
                is_dead_candidate: row.get::<_, bool>(13).unwrap_or(false),
                dead_reason: row.get::<_, Option<String>>(14).unwrap_or(None),
                complexity: row.get::<_, f64>(15).unwrap_or(0.0),
                is_test_file: row.get::<_, i64>(16).map(|v| v != 0).unwrap_or(false),
                test_count: row.get::<_, i64>(17).unwrap_or(0),
                is_tested: row.get::<_, i64>(18).map(|v| v != 0).unwrap_or(false),
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
            "SELECT id, kind, name, path, line_start, line_end, language, churn, coupling, community, in_degree, out_degree, COALESCE(exported, false), COALESCE(is_dead_candidate, false), dead_reason, COALESCE(complexity, 0.0), COALESCE(is_test_file, 0), COALESCE(test_count, 0), COALESCE(is_tested, 0)
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
                exported: row.get::<_, i64>(12).map(|v| v != 0).unwrap_or(false),
                is_dead_candidate: row.get::<_, bool>(13).unwrap_or(false),
                dead_reason: row.get::<_, Option<String>>(14).unwrap_or(None),
                complexity: row.get::<_, f64>(15).unwrap_or(0.0),
                is_test_file: row.get::<_, i64>(16).map(|v| v != 0).unwrap_or(false),
                test_count: row.get::<_, i64>(17).unwrap_or(0),
                is_tested: row.get::<_, i64>(18).map(|v| v != 0).unwrap_or(false),
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

        let mut community_map: std::collections::HashMap<i64, CommunityGroup> =
            std::collections::HashMap::new();
        for row in rows {
            let (community, kind, name, _path, in_degree) = row?;
            let entry = community_map
                .entry(community)
                .or_insert_with(|| (Vec::new(), 0));
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
                let label = top_nodes
                    .first()
                    .cloned()
                    .unwrap_or_else(|| format!("community-{}", community));
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
                    "SELECT DISTINCT n.id, n.kind, n.name, n.path, n.line_start, n.line_end, n.language, n.churn, n.coupling, n.community, n.in_degree, n.out_degree, COALESCE(n.exported, false), COALESCE(n.is_dead_candidate, false), n.dead_reason, COALESCE(n.complexity, 0.0), COALESCE(n.is_test_file, 0), COALESCE(n.test_count, 0), COALESCE(n.is_tested, 0)
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
                        exported: row.get::<_, i64>(12).map(|v| v != 0).unwrap_or(false),
                        is_dead_candidate: row.get::<_, bool>(13).unwrap_or(false),
                        dead_reason: row.get::<_, Option<String>>(14).unwrap_or(None),
                        complexity: row.get::<_, f64>(15).unwrap_or(0.0),
                        is_test_file: row.get::<_, i64>(16).map(|v| v != 0).unwrap_or(false),
                        test_count: row.get::<_, i64>(17).unwrap_or(0),
                        is_tested: row.get::<_, i64>(18).map(|v| v != 0).unwrap_or(false),
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
            "SELECT id, kind, name, path, line_start, line_end, language, churn, coupling, community, in_degree, out_degree, COALESCE(exported, false), COALESCE(is_dead_candidate, false), dead_reason, COALESCE(complexity, 0.0), COALESCE(is_test_file, 0), COALESCE(test_count, 0), COALESCE(is_tested, 0) FROM nodes WHERE community = ?",
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
                exported: row.get::<_, i64>(12).map(|v| v != 0).unwrap_or(false),
                is_dead_candidate: row.get::<_, bool>(13).unwrap_or(false),
                dead_reason: row.get::<_, Option<String>>(14).unwrap_or(None),
                complexity: row.get::<_, f64>(15).unwrap_or(0.0),
                is_test_file: row.get::<_, i64>(16).map(|v| v != 0).unwrap_or(false),
                test_count: row.get::<_, i64>(17).unwrap_or(0),
                is_tested: row.get::<_, i64>(18).map(|v| v != 0).unwrap_or(false),
            })
        })?;
        let mut nodes = Vec::new();
        for row in rows {
            nodes.push(row?);
        }
        Ok(nodes)
    }

    pub fn mark_dead_candidates(&self, items: &[(String, String)]) -> anyhow::Result<()> {
        // items = vec of (node_id, dead_reason)
        if items.is_empty() {
            return Ok(());
        }
        let mut stmt = self
            .conn
            .prepare("UPDATE nodes SET is_dead_candidate = 1, dead_reason = ? WHERE id = ?")?;
        for (id, reason) in items {
            stmt.execute(params![reason, id])?;
        }
        Ok(())
    }

    pub fn get_dead_code_stats(&self) -> anyhow::Result<(i64, i64)> {
        // Returns (total_candidates, high_confidence_count)
        let total: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM nodes WHERE is_dead_candidate = 1",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        // High confidence = unreachable or disconnected reasons
        let high: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM nodes WHERE is_dead_candidate = 1 AND dead_reason IN ('unreachable', 'disconnected')", [], |r| r.get(0)
        ).unwrap_or(0);
        Ok((total, high))
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
        let params: Vec<&dyn duckdb::ToSql> =
            paths.iter().map(|p| p as &dyn duckdb::ToSql).collect();
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
        let params_edges: Vec<&dyn duckdb::ToSql> = paths
            .iter()
            .chain(paths.iter())
            .map(|p| p as &dyn duckdb::ToSql)
            .collect();
        stmt_edges.execute(params_edges.as_slice())?;

        // Delete nodes
        let sql_nodes = format!("DELETE FROM nodes WHERE path IN ({})", placeholders);
        let mut stmt_nodes = self.conn.prepare(&sql_nodes)?;
        let params_nodes: Vec<&dyn duckdb::ToSql> =
            paths.iter().map(|p| p as &dyn duckdb::ToSql).collect();
        let count = stmt_nodes.execute(params_nodes.as_slice())?;
        Ok(count)
    }

    pub fn update_node_doc_comment(&self, id: &str, doc: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE nodes SET doc_comment = ? WHERE id = ?",
            params![doc, id],
        )?;
        Ok(())
    }

    pub fn update_node_complexity(&self, id: &str, complexity: f64) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE nodes SET complexity = ? WHERE id = ?",
            params![complexity, id],
        )?;
        Ok(())
    }

    pub fn get_nodes_by_complexity(
        &self,
        limit: usize,
        min_score: f64,
    ) -> anyhow::Result<Vec<Node>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, name, path, line_start, line_end, language, churn, coupling, community, in_degree, out_degree, COALESCE(exported, false), COALESCE(is_dead_candidate, false), dead_reason, COALESCE(complexity, 0.0), COALESCE(is_test_file, 0), COALESCE(test_count, 0), COALESCE(is_tested, 0)
             FROM nodes
             WHERE kind = 'Function' AND COALESCE(complexity, 0.0) >= ?
             ORDER BY complexity DESC
             LIMIT ?",
        )?;
        let rows = stmt.query_map(params![min_score, limit as i64], |row| {
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
                exported: row.get::<_, i64>(12).map(|v| v != 0).unwrap_or(false),
                is_dead_candidate: row.get::<_, bool>(13).unwrap_or(false),
                dead_reason: row.get::<_, Option<String>>(14).unwrap_or(None),
                complexity: row.get::<_, f64>(15).unwrap_or(0.0),
                is_test_file: row.get::<_, i64>(16).map(|v| v != 0).unwrap_or(false),
                test_count: row.get::<_, i64>(17).unwrap_or(0),
                is_tested: row.get::<_, i64>(18).map(|v| v != 0).unwrap_or(false),
            })
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Returns (overall_pct, Vec<(community_id, documented, total)>, Vec<undocumented high-coupling nodes>)
    pub fn get_docs_coverage(
        &self,
    ) -> anyhow::Result<DocsCoverage> {
        let overall: f64 = self
            .conn
            .query_row(
                "SELECT COALESCE(
                    CAST(SUM(CASE WHEN doc_comment IS NOT NULL AND doc_comment != '' THEN 1 ELSE 0 END) AS DOUBLE)
                    / NULLIF(CAST(COUNT(*) AS DOUBLE), 0.0) * 100.0,
                    0.0)
                 FROM nodes WHERE kind IN ('Function', 'Class') AND path NOT LIKE '%test%'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0.0);

        let mut by_community = Vec::new();
        let mut stmt = self.conn.prepare(
            "SELECT community,
                    SUM(CASE WHEN doc_comment IS NOT NULL AND doc_comment != '' THEN 1 ELSE 0 END) as documented,
                    COUNT(*) as total
             FROM nodes
             WHERE kind IN ('Function', 'Class') AND path NOT LIKE '%test%'
             GROUP BY community
             ORDER BY community",
        )?;
        let comm_rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?;
        for row in comm_rows {
            by_community.push(row?);
        }

        let mut undoc_stmt = self.conn.prepare(
            "SELECT id, kind, name, path, line_start, line_end, language, churn, coupling, community, in_degree, out_degree, COALESCE(exported, false), COALESCE(is_dead_candidate, false), dead_reason, COALESCE(complexity, 0.0), COALESCE(is_test_file, 0), COALESCE(test_count, 0), COALESCE(is_tested, 0)
             FROM nodes
             WHERE kind = 'Function' AND (doc_comment IS NULL OR doc_comment = '')
             ORDER BY in_degree DESC
             LIMIT 10",
        )?;
        let undoc_rows = undoc_stmt.query_map([], |row| {
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
                exported: row.get::<_, i64>(12).map(|v| v != 0).unwrap_or(false),
                is_dead_candidate: row.get::<_, bool>(13).unwrap_or(false),
                dead_reason: row.get::<_, Option<String>>(14).unwrap_or(None),
                complexity: row.get::<_, f64>(15).unwrap_or(0.0),
                is_test_file: row.get::<_, i64>(16).map(|v| v != 0).unwrap_or(false),
                test_count: row.get::<_, i64>(17).unwrap_or(0),
                is_tested: row.get::<_, i64>(18).map(|v| v != 0).unwrap_or(false),
            })
        })?;
        let mut undocumented = Vec::new();
        for row in undoc_rows {
            undocumented.push(row?);
        }

        Ok((overall, by_community, undocumented))
    }

    pub fn upsert_clones(&self, clones: &[CloneRow]) -> anyhow::Result<usize> {
        if clones.is_empty() {
            return Ok(0);
        }
        let mut count = 0;
        let mut stmt = self.conn.prepare(
            "INSERT OR REPLACE INTO clones (id, node_a, node_b, similarity, kind) VALUES (?, ?, ?, ?, ?)",
        )?;
        for c in clones {
            stmt.execute(params![c.id, c.node_a, c.node_b, c.similarity, c.kind])?;
            count += 1;
        }
        Ok(count)
    }

    pub fn get_clones(
        &self,
        min_similarity: f64,
        kind_filter: Option<&str>,
    ) -> anyhow::Result<Vec<CloneRow>> {
        let (sql, use_kind) = if kind_filter.is_some() {
            (
                "SELECT id, node_a, node_b, similarity, kind FROM clones WHERE similarity >= ? AND kind = ? ORDER BY similarity DESC",
                true,
            )
        } else {
            (
                "SELECT id, node_a, node_b, similarity, kind FROM clones WHERE similarity >= ? ORDER BY similarity DESC",
                false,
            )
        };

        let mut stmt = self.conn.prepare(sql)?;
        let map_row = |row: &duckdb::Row| {
            Ok(CloneRow {
                id: row.get(0)?,
                node_a: row.get(1)?,
                node_b: row.get(2)?,
                similarity: row.get::<_, f32>(3)? as f64,
                kind: row.get(4)?,
            })
        };

        let rows = if use_kind {
            stmt.query_map(
                params![min_similarity, kind_filter.unwrap_or("")],
                map_row,
            )?
        } else {
            stmt.query_map(params![min_similarity], map_row)?
        };

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    pub fn clear_clones(&self) -> anyhow::Result<()> {
        self.conn.execute("DELETE FROM clones", [])?;
        Ok(())
    }

    pub fn mark_test_files(&self, paths: &[String]) -> anyhow::Result<()> {
        if paths.is_empty() {
            return Ok(());
        }
        let mut stmt = self
            .conn
            .prepare("UPDATE nodes SET is_test_file = 1 WHERE path = ?")?;
        for path in paths {
            stmt.execute(params![path])?;
        }
        Ok(())
    }

    /// After inserting TESTS edges, compute test_count and is_tested for non-test nodes.
    pub fn update_test_coverage(&self) -> anyhow::Result<()> {
        self.conn.execute_batch(
            "UPDATE nodes SET test_count = (
                SELECT COUNT(*) FROM edges
                WHERE edges.dst = nodes.id AND edges.kind = 'TESTS'
             );
             UPDATE nodes SET is_tested = (test_count > 0)
             WHERE is_test_file = 0;",
        )?;
        Ok(())
    }

    /// Returns (overall_pct, tested_count, untested_count, gaps ranked by risk)
    pub fn get_test_coverage_summary(
        &self,
        top_n: usize,
    ) -> anyhow::Result<(f64, i64, i64, Vec<Node>)> {
        let tested: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM nodes WHERE kind IN ('Function','Class') AND is_test_file = 0 AND is_tested = 1",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let total: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM nodes WHERE kind IN ('Function','Class') AND is_test_file = 0",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        let overall_pct = if total > 0 {
            (tested as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        let mut gap_stmt = self.conn.prepare(
            "SELECT id, kind, name, path, line_start, line_end, language, churn, coupling, community, in_degree, out_degree, COALESCE(exported, false), COALESCE(is_dead_candidate, false), dead_reason, COALESCE(complexity, 0.0), COALESCE(is_test_file, 0), COALESCE(test_count, 0), COALESCE(is_tested, 0)
             FROM nodes
             WHERE kind IN ('Function','Class') AND is_test_file = 0 AND COALESCE(is_tested, 0) = 0
             ORDER BY (churn * CAST(in_degree AS DOUBLE) + CAST(in_degree AS DOUBLE) * 0.5) DESC
             LIMIT ?",
        )?;
        let gap_rows = gap_stmt.query_map(params![top_n as i64], |row| {
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
                exported: row.get::<_, i64>(12).map(|v| v != 0).unwrap_or(false),
                is_dead_candidate: row.get::<_, bool>(13).unwrap_or(false),
                dead_reason: row.get::<_, Option<String>>(14).unwrap_or(None),
                complexity: row.get::<_, f64>(15).unwrap_or(0.0),
                is_test_file: row.get::<_, i64>(16).map(|v| v != 0).unwrap_or(false),
                test_count: row.get::<_, i64>(17).unwrap_or(0),
                is_tested: row.get::<_, i64>(18).map(|v| v != 0).unwrap_or(false),
            })
        })?;
        let mut gaps = Vec::new();
        for row in gap_rows {
            gaps.push(row?);
        }

        Ok((overall_pct, tested, total - tested, gaps))
    }

    pub fn upsert_snapshot(&self, entry: &SnapshotEntry) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO snapshots (id, commit_sha, commit_date, commit_msg, node_count, edge_count, snapshot_data)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![
                entry.id,
                entry.commit_sha,
                entry.commit_date,
                entry.commit_msg,
                entry.node_count,
                entry.edge_count,
                entry.snapshot_data,
            ],
        )?;
        Ok(())
    }

    pub fn get_snapshots(&self, limit: usize) -> anyhow::Result<Vec<SnapshotEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, commit_sha, commit_date, commit_msg, COALESCE(node_count,0), COALESCE(edge_count,0), snapshot_data
             FROM snapshots ORDER BY commit_date DESC LIMIT ?",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(SnapshotEntry {
                id: row.get(0)?,
                commit_sha: row.get(1)?,
                commit_date: row.get(2)?,
                commit_msg: row.get(3)?,
                node_count: row.get(4)?,
                edge_count: row.get(5)?,
                snapshot_data: row.get(6)?,
            })
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    pub fn get_snapshot_by_sha(&self, sha: &str) -> anyhow::Result<Option<SnapshotEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, commit_sha, commit_date, commit_msg, COALESCE(node_count,0), COALESCE(edge_count,0), snapshot_data
             FROM snapshots WHERE commit_sha = ? OR commit_sha LIKE ? LIMIT 1",
        )?;
        let prefix = format!("{}%", sha);
        let mut rows = stmt.query_map(params![sha, prefix], |row| {
            Ok(SnapshotEntry {
                id: row.get(0)?,
                commit_sha: row.get(1)?,
                commit_date: row.get(2)?,
                commit_msg: row.get(3)?,
                node_count: row.get(4)?,
                edge_count: row.get(5)?,
                snapshot_data: row.get(6)?,
            })
        })?;
        match rows.next() {
            Some(Ok(entry)) => Ok(Some(entry)),
            _ => Ok(None),
        }
    }

    pub fn snapshot_count(&self) -> i64 {
        self.conn
            .query_row("SELECT COUNT(*) FROM snapshots", [], |r| r.get(0))
            .unwrap_or(0)
    }
}

pub fn repo_hash(path: &Path) -> String {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let path_str = canonical.to_string_lossy().to_string();
    let mut hasher = Sha256::new();
    hasher.update(path_str.as_bytes());
    format!("{:x}", hasher.finalize())[..16].to_string()
}
