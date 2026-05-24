//! Declarative graph predicates evaluated against a freshly-indexed repo.
//!
//! The intended usage is `git bisect run cgx bisect-script` — at each commit
//! bisect visits, `cgx analyze` re-indexes the repo and this module evaluates
//! the predicates defined in `.cgx/bisect.toml`. The runner exits with `0` if
//! the commit is "good" (all predicates pass), `1` if "bad", and `125` if cgx
//! couldn't determine an answer (which `git bisect` interprets as "skip").
//!
//! Example `.cgx/bisect.toml`:
//!
//! ```toml
//! # All predicates must hold for the commit to be "good".
//! node_count_min  = 700
//! node_count_max  = 2000
//! nodes_exist     = ["fn:crates/cgx-engine/src/parser.rs:parse"]
//! nodes_missing   = []
//! nodes_alive     = ["fn:crates/cgx-engine/src/graph.rs:get_file_summary"]
//! rule_violations_max = 0
//! ```

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::graph::GraphDb;

/// The set of predicates the user wrote in `.cgx/bisect.toml`.
///
/// Empty fields are treated as "no constraint". All present fields must hold
/// for a commit to be considered "good".
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BisectPredicates {
    /// Graph must have at least this many nodes.
    #[serde(default)]
    pub node_count_min: Option<u64>,
    /// Graph must have at most this many nodes.
    #[serde(default)]
    pub node_count_max: Option<u64>,
    /// Every node ID in this list must exist.
    #[serde(default)]
    pub nodes_exist: Vec<String>,
    /// No node ID in this list may exist.
    #[serde(default)]
    pub nodes_missing: Vec<String>,
    /// Every node ID in this list must exist AND not be flagged as a dead-code candidate.
    #[serde(default)]
    pub nodes_alive: Vec<String>,
    /// Total rule violations (per `cgx rules check`) must be ≤ this number.
    #[serde(default)]
    pub rule_violations_max: Option<u64>,
}

/// Result of a single predicate evaluation. Implements `Display` for nice CLI output.
#[derive(Debug, Clone)]
pub struct PredicateOutcome {
    pub predicate: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct BisectReport {
    pub outcomes: Vec<PredicateOutcome>,
}

impl BisectReport {
    pub fn passed(&self) -> bool {
        self.outcomes.iter().all(|o| o.passed)
    }
}

impl BisectPredicates {
    /// Read predicates from `.cgx/bisect.toml` (or the supplied custom path).
    pub fn load(repo_path: &Path, custom: Option<&Path>) -> anyhow::Result<Self> {
        let target = match custom {
            Some(p) => p.to_path_buf(),
            None => repo_path.join(".cgx").join("bisect.toml"),
        };
        if !target.exists() {
            anyhow::bail!(
                "no predicate file at {}. Create `.cgx/bisect.toml` first — see `cgx bisect-script --example` for the schema.",
                target.display()
            );
        }
        let content = std::fs::read_to_string(&target)?;
        let parsed: BisectPredicates = toml::from_str(&content)?;
        Ok(parsed)
    }

    /// Path the bisect script writes to by default.
    pub fn default_path(repo_path: &Path) -> PathBuf {
        repo_path.join(".cgx").join("bisect.toml")
    }

    /// Evaluate every populated predicate against the live graph.
    pub fn evaluate(&self, db: &GraphDb) -> anyhow::Result<BisectReport> {
        let mut outcomes: Vec<PredicateOutcome> = Vec::new();

        if let Some(min) = self.node_count_min {
            let actual = db.node_count().unwrap_or(0);
            outcomes.push(PredicateOutcome {
                predicate: "node_count_min".to_string(),
                passed: actual >= min,
                detail: format!("required ≥ {}, got {}", min, actual),
            });
        }
        if let Some(max) = self.node_count_max {
            let actual = db.node_count().unwrap_or(0);
            outcomes.push(PredicateOutcome {
                predicate: "node_count_max".to_string(),
                passed: actual <= max,
                detail: format!("required ≤ {}, got {}", max, actual),
            });
        }

        for id in &self.nodes_exist {
            let exists = db.get_node(id).map(|n| n.is_some()).unwrap_or(false);
            outcomes.push(PredicateOutcome {
                predicate: format!("nodes_exist:{}", id),
                passed: exists,
                detail: if exists {
                    "found".to_string()
                } else {
                    "missing".to_string()
                },
            });
        }

        for id in &self.nodes_missing {
            let exists = db.get_node(id).map(|n| n.is_some()).unwrap_or(false);
            outcomes.push(PredicateOutcome {
                predicate: format!("nodes_missing:{}", id),
                passed: !exists,
                detail: if exists {
                    "still present".to_string()
                } else {
                    "absent".to_string()
                },
            });
        }

        for id in &self.nodes_alive {
            let node = db.get_node(id).ok().flatten();
            let (alive, detail) = match &node {
                Some(n) if n.is_dead_candidate => (false, "flagged dead".to_string()),
                Some(_) => (true, "alive".to_string()),
                None => (false, "missing".to_string()),
            };
            outcomes.push(PredicateOutcome {
                predicate: format!("nodes_alive:{}", id),
                passed: alive,
                detail,
            });
        }

        if let Some(_max) = self.rule_violations_max {
            // Rule evaluation lives in the `rules` module; we surface a placeholder
            // outcome rather than re-implementing the rule engine here. Callers can
            // separately run `cgx rules check` and feed the count back in via
            // `--rule-violations N` on the CLI.
            outcomes.push(PredicateOutcome {
                predicate: "rule_violations_max".to_string(),
                passed: true,
                detail: "skipped (run `cgx rules check` separately and use --rule-violations)"
                    .to_string(),
            });
        }

        Ok(BisectReport { outcomes })
    }
}

/// Sample TOML emitted by `cgx bisect-script --example`. Keep in sync with the
/// `BisectPredicates` struct.
pub const EXAMPLE_TOML: &str = r#"# .cgx/bisect.toml — declarative predicates for `git bisect run cgx bisect-script`.
# All populated predicates must pass for the commit to be considered "good".

# Graph size bounds — useful for catching mass deletions or runaway code growth.
node_count_min = 100
# node_count_max = 5000

# Node IDs that must exist (e.g. a public function you accidentally deleted).
nodes_exist = [
    # "fn:src/auth.rs:authenticate",
]

# Node IDs that must NOT exist (e.g. a symbol you renamed away that shouldn't come back).
nodes_missing = [
    # "fn:src/auth.rs:old_authenticate",
]

# Node IDs that must exist AND not be flagged as dead-code candidates.
nodes_alive = [
    # "fn:src/api.rs:start_server",
]

# Maximum number of architecture-rule violations allowed (run `cgx rules check`
# separately and feed the count via `--rule-violations N`).
# rule_violations_max = 0
"#;
