use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Metadata for an indexed repository stored in the global registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoEntry {
    /// Stable SHA-256–derived ID for the repo path.
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    /// Path to the DuckDB database file (`~/.cgx/repos/<id>.db`).
    pub db_path: PathBuf,
    pub indexed_at: String,
    pub node_count: u64,
    pub edge_count: u64,
    /// Fraction of nodes per language, e.g. `{"typescript": 0.72, "rust": 0.28}`.
    #[serde(default)]
    pub language_breakdown: HashMap<String, f64>,
}

/// Global registry of all repositories indexed by cgx, persisted at `~/.cgx/registry.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registry {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub repos: Vec<RepoEntry>,
}

fn default_version() -> u32 {
    1
}

impl Registry {
    fn path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".cgx")
            .join("registry.json")
    }

    /// Load the registry from `~/.cgx/registry.json`, creating it if absent.
    pub fn load() -> anyhow::Result<Self> {
        let path = Self::path();
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let registry: Registry = serde_json::from_str(&content)?;
            Ok(registry)
        } else {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            Ok(Registry {
                version: 1,
                repos: Vec::new(),
            })
        }
    }

    /// Persist the registry to `~/.cgx/registry.json`.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Add or replace a repo entry (matched by `id`).
    pub fn register(&mut self, entry: RepoEntry) {
        self.repos.retain(|r| r.id != entry.id);
        self.repos.push(entry);
    }

    /// Look up a repo by its canonical on-disk path.
    pub fn find_by_path(&self, path: &Path) -> Option<&RepoEntry> {
        let canonical = path.canonicalize().ok()?;
        self.repos
            .iter()
            .find(|r| r.path.canonicalize().ok().as_ref() == Some(&canonical))
    }

    /// Look up a repo by its stable SHA-derived `id`.
    pub fn find_by_id(&self, id: &str) -> Option<&RepoEntry> {
        self.repos.iter().find(|r| r.id == id)
    }
}
