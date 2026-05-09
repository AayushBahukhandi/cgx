use crate::graph::{GraphDb, Node};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeadNode {
    pub node: Node,
    pub reason: DeadReason,
    pub confidence: Confidence,
    pub false_positive_risk: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

impl Confidence {
    pub fn as_str(&self) -> &'static str {
        match self {
            Confidence::High => "high",
            Confidence::Medium => "medium",
            Confidence::Low => "low",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeadReason {
    UnreferencedExport,
    Unreachable,
    UnusedVariable,
    Disconnected,
    ZombieFile,
}

impl DeadReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            DeadReason::UnreferencedExport => "unreferenced_export",
            DeadReason::Unreachable => "unreachable",
            DeadReason::UnusedVariable => "unused_variable",
            DeadReason::Disconnected => "disconnected",
            DeadReason::ZombieFile => "zombie_file",
        }
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DeadCodeReport {
    pub unreferenced_exports: Vec<DeadNode>,
    pub unreachable: Vec<DeadNode>,
    pub unused_variables: Vec<DeadNode>,
    pub disconnected: Vec<DeadNode>,
    pub zombie_files: Vec<DeadNode>,
}

impl DeadCodeReport {
    pub fn all_items(&self) -> Vec<&DeadNode> {
        let mut all = Vec::new();
        all.extend(self.unreferenced_exports.iter());
        all.extend(self.unreachable.iter());
        all.extend(self.unused_variables.iter());
        all.extend(self.disconnected.iter());
        all.extend(self.zombie_files.iter());
        all
    }

    pub fn total(&self) -> usize {
        self.unreferenced_exports.len()
            + self.unreachable.len()
            + self.unused_variables.len()
            + self.disconnected.len()
            + self.zombie_files.len()
    }

    pub fn count_by_confidence(&self) -> (usize, usize, usize) {
        let mut high = 0;
        let mut medium = 0;
        let mut low = 0;
        for item in self.all_items() {
            match item.confidence {
                Confidence::High => high += 1,
                Confidence::Medium => medium += 1,
                Confidence::Low => low += 1,
            }
        }
        (high, medium, low)
    }
}

fn query_nodes(db: &GraphDb, sql: &str) -> anyhow::Result<Vec<Node>> {
    let mut stmt = db.conn.prepare(sql)?;
    let rows = stmt.query_map([], |row| {
        Ok(Node {
            id: row.get(0)?,
            kind: row.get(1)?,
            name: row.get(2)?,
            path: row.get(3)?,
            line_start: row.get::<_, u32>(4)?,
            line_end: row.get::<_, u32>(5)?,
            language: row.get::<_, Option<String>>(6)?.unwrap_or_default(),
            churn: row.get::<_, f64>(7)?,
            coupling: row.get::<_, f64>(8)?,
            community: row.get::<_, i64>(9)?,
            in_degree: row.get::<_, i64>(10)?,
            out_degree: row.get::<_, i64>(11)?,
            exported: row.get::<_, i64>(12).map(|v| v != 0).unwrap_or(false),
            is_dead_candidate: false,
            dead_reason: None,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

const FRAMEWORK_HOOKS: &[&str] = &[
    "getServerSideProps",
    "getStaticProps",
    "loader",
    "action",
    "beforeEach",
    "afterAll",
    "getStaticPaths",
];

const ENTRY_POINT_NAMES: &[&str] = &["main", "init", "setup", "bootstrap", "start"];

fn compute_confidence_and_fp(node: &Node, reason: &DeadReason) -> (Confidence, Option<String>) {
    // Framework hooks -> Low
    if FRAMEWORK_HOOKS.contains(&node.name.as_str()) {
        return (
            Confidence::Low,
            Some("Framework hook — called by framework not by your code".to_string()),
        );
    }
    // Entry point names
    if ENTRY_POINT_NAMES.contains(&node.name.as_str()) {
        return (
            Confidence::Low,
            Some("Common entry point name — verify before deleting".to_string()),
        );
    }
    // Types/interfaces erased at runtime
    if node.kind == "Type" || node.kind == "Interface" {
        return (
            Confidence::Low,
            Some(
                "Types erased at runtime — may be used by consuming TypeScript packages"
                    .to_string(),
            ),
        );
    }
    // lib/ or dist/ files
    if node.path.contains("/lib/")
        || node.path.contains("/dist/")
        || node.path.starts_with("lib/")
        || node.path.starts_with("dist/")
    {
        return (
            Confidence::Low,
            Some("May be consumed externally by npm consumers".to_string()),
        );
    }

    match reason {
        DeadReason::Unreachable => (Confidence::High, None),
        DeadReason::Disconnected => (Confidence::High, None),
        DeadReason::UnreferencedExport => {
            let filename = node.path.split('/').next_back().unwrap_or("");
            if matches!(filename, "index.ts" | "index.js" | "lib.rs" | "mod.rs") {
                (
                    Confidence::Low,
                    Some("May be consumed externally by npm consumers".to_string()),
                )
            } else {
                (Confidence::High, None)
            }
        }
        DeadReason::UnusedVariable => (Confidence::Medium, None),
        DeadReason::ZombieFile => (Confidence::Medium, None),
    }
}

pub fn detect_dead_code(db: &GraphDb) -> anyhow::Result<DeadCodeReport> {
    let mut report = DeadCodeReport::default();

    // Query 1: unreferenced exports — exported but no CALLS edges pointing at them.
    // Uses NOT EXISTS on CALLS rather than in_degree=0 because all parsed nodes
    // receive an EXPORTS edge from their containing file, making in_degree >= 1.
    let unreferenced_exports = query_nodes(
        db,
        "SELECT n.id, n.kind, n.name, n.path, n.line_start, n.line_end, n.language, n.churn,
                n.coupling, n.community, n.in_degree, n.out_degree, COALESCE(n.exported, 0)
         FROM nodes n
         WHERE n.kind IN ('Function','Class','Variable','Type')
         AND COALESCE(n.exported, 0) = 1
         AND NOT EXISTS (SELECT 1 FROM edges e WHERE e.dst = n.id AND e.kind = 'CALLS')
         AND n.path NOT LIKE '%test%' AND n.path NOT LIKE '%spec%'
         AND n.path NOT LIKE '%.d.ts'",
    )?;

    for node in unreferenced_exports {
        let (confidence, fp_risk) =
            compute_confidence_and_fp(&node, &DeadReason::UnreferencedExport);
        report.unreferenced_exports.push(DeadNode {
            node,
            reason: DeadReason::UnreferencedExport,
            confidence,
            false_positive_risk: fp_risk,
        });
    }

    // Query 2: unreachable private functions — not exported, no CALLS edges to them.
    let unreachable = query_nodes(
        db,
        "SELECT n.id, n.kind, n.name, n.path, n.line_start, n.line_end, n.language, n.churn,
                n.coupling, n.community, n.in_degree, n.out_degree, COALESCE(n.exported, 0)
         FROM nodes n
         WHERE n.kind = 'Function'
         AND n.name != 'constructor'
         AND COALESCE(n.exported, 0) = 0
         AND NOT EXISTS (SELECT 1 FROM edges e WHERE e.dst = n.id AND e.kind = 'CALLS')
         AND n.path NOT LIKE '%test%'",
    )?;

    for node in unreachable {
        let (confidence, fp_risk) = compute_confidence_and_fp(&node, &DeadReason::Unreachable);
        report.unreachable.push(DeadNode {
            node,
            reason: DeadReason::Unreachable,
            confidence,
            false_positive_risk: fp_risk,
        });
    }

    // Query 3: unused variables — no CALLS edges point to them.
    let unused_vars = query_nodes(
        db,
        "SELECT n.id, n.kind, n.name, n.path, n.line_start, n.line_end, n.language, n.churn,
                n.coupling, n.community, n.in_degree, n.out_degree, COALESCE(n.exported, 0)
         FROM nodes n
         WHERE n.kind = 'Variable'
         AND NOT EXISTS (SELECT 1 FROM edges e WHERE e.dst = n.id AND e.kind = 'CALLS')
         AND n.path NOT LIKE '%test%'",
    )?;

    for node in unused_vars {
        let (confidence, fp_risk) = compute_confidence_and_fp(&node, &DeadReason::UnusedVariable);
        report.unused_variables.push(DeadNode {
            node,
            reason: DeadReason::UnusedVariable,
            confidence,
            false_positive_risk: fp_risk,
        });
    }

    // Query 4: disconnected nodes — no CALLS edges in either direction.
    let disconnected = query_nodes(
        db,
        "SELECT n.id, n.kind, n.name, n.path, n.line_start, n.line_end, n.language, n.churn,
                n.coupling, n.community, n.in_degree, n.out_degree, COALESCE(n.exported, 0)
         FROM nodes n
         WHERE NOT EXISTS (SELECT 1 FROM edges e WHERE e.dst = n.id AND e.kind = 'CALLS')
         AND NOT EXISTS (SELECT 1 FROM edges e WHERE e.src = n.id AND e.kind = 'CALLS')
         AND n.kind NOT IN ('File','Module','Author')
         AND n.path NOT LIKE '%test%'",
    )?;

    // Collect IDs already in other categories to avoid duplication
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for dn in &report.unreferenced_exports {
        seen.insert(dn.node.id.clone());
    }
    for dn in &report.unreachable {
        seen.insert(dn.node.id.clone());
    }
    for dn in &report.unused_variables {
        seen.insert(dn.node.id.clone());
    }

    for node in disconnected {
        if seen.contains(&node.id) {
            continue;
        }
        let (confidence, fp_risk) = compute_confidence_and_fp(&node, &DeadReason::Disconnected);
        report.disconnected.push(DeadNode {
            node,
            reason: DeadReason::Disconnected,
            confidence,
            false_positive_risk: fp_risk,
        });
    }

    // Query 5: zombie files — no IMPORTS edges point to them, but they export something.
    // File in_degree = count of IMPORTS edges from other files, so 0 means never imported.
    let zombie_files = query_nodes(
        db,
        "SELECT n.id, n.kind, n.name, n.path, n.line_start, n.line_end, n.language, n.churn,
                n.coupling, n.community, n.in_degree, n.out_degree, COALESCE(n.exported, 0)
         FROM nodes n
         WHERE n.kind = 'File'
         AND NOT EXISTS (SELECT 1 FROM edges e WHERE e.dst = n.id AND e.kind = 'IMPORTS')
         AND EXISTS (SELECT 1 FROM edges e WHERE e.src = n.id)
         AND regexp_extract(n.name, '[^/]+$') NOT IN ('index.ts','index.js','main.ts','main.rs','lib.rs','mod.rs','app.ts','__init__.py')
         AND n.path NOT LIKE '%test%'",
    )?;

    for node in zombie_files {
        let (confidence, fp_risk) = compute_confidence_and_fp(&node, &DeadReason::ZombieFile);
        report.zombie_files.push(DeadNode {
            node,
            reason: DeadReason::ZombieFile,
            confidence,
            false_positive_risk: fp_risk,
        });
    }

    Ok(report)
}

pub fn mark_dead_candidates(db: &GraphDb, report: &DeadCodeReport) -> anyhow::Result<()> {
    let mut items: Vec<(String, String)> = Vec::new();
    for dn in &report.unreferenced_exports {
        items.push((dn.node.id.clone(), dn.reason.as_str().to_string()));
    }
    for dn in &report.unreachable {
        items.push((dn.node.id.clone(), dn.reason.as_str().to_string()));
    }
    for dn in &report.unused_variables {
        items.push((dn.node.id.clone(), dn.reason.as_str().to_string()));
    }
    for dn in &report.disconnected {
        items.push((dn.node.id.clone(), dn.reason.as_str().to_string()));
    }
    for dn in &report.zombie_files {
        items.push((dn.node.id.clone(), dn.reason.as_str().to_string()));
    }
    db.mark_dead_candidates(&items)
}
