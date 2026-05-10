use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub severity: String,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub built_in: Option<String>,
    #[serde(default)]
    pub threshold: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesConfig {
    #[serde(default)]
    pub rules: Vec<Rule>,
}

impl RulesConfig {
    pub fn load(repo_root: &Path) -> anyhow::Result<Self> {
        let rules_path = repo_root.join(".cgx").join("rules.toml");
        if !rules_path.exists() {
            return Ok(Self { rules: Vec::new() });
        }
        let content = std::fs::read_to_string(&rules_path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }
}

#[derive(Debug, Clone)]
pub struct RuleViolation {
    pub rule_name: String,
    pub severity: String,
    pub message: String,
    pub file: Option<String>,
    pub line: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct RuleResult {
    pub rule: Rule,
    pub violations: Vec<RuleViolation>,
    pub error: Option<String>,
}

impl RuleResult {
    pub fn passed(&self) -> bool {
        self.violations.is_empty() && self.error.is_none()
    }
}

pub fn run_rules(
    db: &crate::graph::GraphDb,
    rules: &[Rule],
    filter_name: Option<&str>,
) -> Vec<RuleResult> {
    rules
        .iter()
        .filter(|r| filter_name.is_none_or(|n| r.name == n))
        .map(|rule| run_single_rule(db, rule))
        .collect()
}

fn run_single_rule(db: &crate::graph::GraphDb, rule: &Rule) -> RuleResult {
    if let Some(ref builtin) = rule.built_in {
        run_builtin_rule(db, rule, builtin)
    } else if let Some(ref query) = rule.query {
        run_sql_rule(db, rule, query)
    } else {
        RuleResult {
            rule: rule.clone(),
            violations: Vec::new(),
            error: Some("Rule has neither 'query' nor 'built_in' key".to_string()),
        }
    }
}

fn run_builtin_rule(db: &crate::graph::GraphDb, rule: &Rule, builtin: &str) -> RuleResult {
    match builtin {
        "no_cycles" => run_no_cycles(db, rule),
        "max_coupling" => run_max_coupling(db, rule),
        "max_complexity" => run_max_complexity(db, rule),
        "require_docs_for_public" => run_require_docs(db, rule),
        _ => RuleResult {
            rule: rule.clone(),
            violations: Vec::new(),
            error: Some(format!("Unknown built-in rule: {}", builtin)),
        },
    }
}

fn run_no_cycles(db: &crate::graph::GraphDb, rule: &Rule) -> RuleResult {
    // Detect cycles using DFS on IMPORTS edges
    let mut stmt = match db
        .conn
        .prepare("SELECT DISTINCT src, dst FROM edges WHERE kind = 'IMPORTS' AND src != dst")
    {
        Ok(s) => s,
        Err(e) => {
            return RuleResult {
                rule: rule.clone(),
                violations: Vec::new(),
                error: Some(format!("Query failed: {}", e)),
            }
        }
    };

    let mapped = match stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) {
        Ok(m) => m,
        Err(_) => {
            return RuleResult {
                rule: rule.clone(),
                violations: Vec::new(),
                error: None,
            }
        }
    };
    let edges: Vec<(String, String)> = mapped.filter_map(|r| r.ok()).collect();

    // Build adjacency list
    let mut adj: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for (src, dst) in &edges {
        adj.entry(src.clone()).or_default().push(dst.clone());
    }

    // DFS cycle detection
    let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut in_stack: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut cycles: Vec<String> = Vec::new();

    let nodes: Vec<String> = adj.keys().cloned().collect();
    for node in &nodes {
        if !visited.contains(node) {
            detect_cycle(node, &adj, &mut visited, &mut in_stack, &mut cycles);
        }
    }

    let violations: Vec<RuleViolation> = cycles
        .into_iter()
        .take(10)
        .map(|cycle| RuleViolation {
            rule_name: rule.name.clone(),
            severity: rule.severity.clone(),
            message: format!("Circular import detected: {}", cycle),
            file: None,
            line: None,
        })
        .collect();

    RuleResult {
        rule: rule.clone(),
        violations,
        error: None,
    }
}

fn detect_cycle(
    node: &str,
    adj: &std::collections::HashMap<String, Vec<String>>,
    visited: &mut std::collections::HashSet<String>,
    in_stack: &mut std::collections::HashSet<String>,
    cycles: &mut Vec<String>,
) {
    visited.insert(node.to_string());
    in_stack.insert(node.to_string());

    if let Some(neighbors) = adj.get(node) {
        for neighbor in neighbors {
            if !visited.contains(neighbor) {
                detect_cycle(neighbor, adj, visited, in_stack, cycles);
            } else if in_stack.contains(neighbor) {
                cycles.push(format!("{} -> {}", node, neighbor));
            }
        }
    }

    in_stack.remove(node);
}

fn run_max_coupling(db: &crate::graph::GraphDb, rule: &Rule) -> RuleResult {
    let threshold = rule.threshold.unwrap_or(30.0) as i64;
    let mut stmt = match db.conn.prepare(
        "SELECT name, path, in_degree FROM nodes WHERE kind != 'Author' AND in_degree > ? ORDER BY in_degree DESC LIMIT 20",
    ) {
        Ok(s) => s,
        Err(e) => {
            return RuleResult {
                rule: rule.clone(),
                violations: Vec::new(),
                error: Some(format!("Query failed: {}", e)),
            }
        }
    };

    let mapped = match stmt.query_map(duckdb::params![threshold], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
        ))
    }) {
        Ok(m) => m,
        Err(e) => {
            return RuleResult {
                rule: rule.clone(),
                violations: Vec::new(),
                error: Some(format!("Query failed: {}", e)),
            }
        }
    };
    let violations: Vec<RuleViolation> = mapped
        .filter_map(|r| r.ok())
        .map(|(name, path, degree)| RuleViolation {
            rule_name: rule.name.clone(),
            severity: rule.severity.clone(),
            message: format!("{} has {} callers (threshold: {})", name, degree, threshold),
            file: Some(path),
            line: None,
        })
        .collect();

    RuleResult {
        rule: rule.clone(),
        violations,
        error: None,
    }
}

fn run_max_complexity(db: &crate::graph::GraphDb, rule: &Rule) -> RuleResult {
    let threshold = rule.threshold.unwrap_or(0.3);
    let mut stmt = match db.conn.prepare(
        "SELECT name, path, complexity FROM nodes WHERE kind = 'Function' AND complexity > ? ORDER BY complexity DESC LIMIT 20",
    ) {
        Ok(s) => s,
        Err(e) => {
            return RuleResult {
                rule: rule.clone(),
                violations: Vec::new(),
                error: Some(format!("Query failed: {}", e)),
            }
        }
    };

    let mapped = match stmt.query_map(duckdb::params![threshold], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, f64>(2)?,
        ))
    }) {
        Ok(m) => m,
        Err(e) => {
            return RuleResult {
                rule: rule.clone(),
                violations: Vec::new(),
                error: Some(format!("Query failed: {}", e)),
            }
        }
    };
    let violations: Vec<RuleViolation> = mapped
        .filter_map(|r| r.ok())
        .map(|(name, path, complexity)| RuleViolation {
            rule_name: rule.name.clone(),
            severity: rule.severity.clone(),
            message: format!(
                "{} has complexity {:.2} (threshold: {:.2})",
                name, complexity, threshold
            ),
            file: Some(path),
            line: None,
        })
        .collect();

    RuleResult {
        rule: rule.clone(),
        violations,
        error: None,
    }
}

fn run_require_docs(db: &crate::graph::GraphDb, rule: &Rule) -> RuleResult {
    let mut stmt = match db.conn.prepare(
        "SELECT name, path FROM nodes WHERE kind IN ('Function','Class') AND exported = 1 AND (doc_comment IS NULL OR doc_comment = '') ORDER BY name LIMIT 50",
    ) {
        Ok(s) => s,
        Err(e) => {
            return RuleResult {
                rule: rule.clone(),
                violations: Vec::new(),
                error: Some(format!("Query failed: {}", e)),
            }
        }
    };

    let mapped = match stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) {
        Ok(m) => m,
        Err(e) => {
            return RuleResult {
                rule: rule.clone(),
                violations: Vec::new(),
                error: Some(format!("Query failed: {}", e)),
            }
        }
    };
    let violations: Vec<RuleViolation> = mapped
        .filter_map(|r| r.ok())
        .map(|(name, path)| RuleViolation {
            rule_name: rule.name.clone(),
            severity: rule.severity.clone(),
            message: format!("Public {} has no doc comment", name),
            file: Some(path),
            line: None,
        })
        .collect();

    RuleResult {
        rule: rule.clone(),
        violations,
        error: None,
    }
}

fn run_sql_rule(db: &crate::graph::GraphDb, rule: &Rule, sql: &str) -> RuleResult {
    let mut stmt = match db.conn.prepare(sql) {
        Ok(s) => s,
        Err(e) => {
            return RuleResult {
                rule: rule.clone(),
                violations: Vec::new(),
                error: Some(format!("SQL error: {}", e)),
            }
        }
    };

    // Execute and collect all rows as Vec<String> — read each column as String
    let rows = stmt.query_map([], |row| {
        // Try to read up to 10 columns; stop at first error
        let mut parts = Vec::new();
        for i in 0..10usize {
            match row.get::<_, Option<String>>(i) {
                Ok(Some(s)) => parts.push(s),
                Ok(None) => parts.push(String::new()),
                Err(_) => {
                    // Try integer
                    match row.get::<_, i64>(i) {
                        Ok(v) => parts.push(v.to_string()),
                        Err(_) => break,
                    }
                }
            }
        }
        Ok(parts)
    });

    let violations: Vec<RuleViolation> = match rows {
        Err(e) => {
            return RuleResult {
                rule: rule.clone(),
                violations: Vec::new(),
                error: Some(format!("Query execution failed: {}", e)),
            }
        }
        Ok(rows) => rows
            .filter_map(|r| r.ok())
            .filter(|cols| !cols.is_empty())
            .map(|cols| {
                let file = cols.first().cloned();
                let message = cols.join(", ");
                RuleViolation {
                    rule_name: rule.name.clone(),
                    severity: rule.severity.clone(),
                    message,
                    file,
                    line: None,
                }
            })
            .collect(),
    };

    RuleResult {
        rule: rule.clone(),
        violations,
        error: None,
    }
}
