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
    /// RFC3339 timestamp of the last time this repo was queried or analyzed.
    /// Falls back to `indexed_at` for entries created before LRU tracking existed.
    #[serde(default)]
    pub last_used_at: Option<String>,
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
    pub fn register(&mut self, mut entry: RepoEntry) {
        if entry.last_used_at.is_none() {
            entry.last_used_at = Some(entry.indexed_at.clone());
        }
        self.repos.retain(|r| r.id != entry.id);
        self.repos.push(entry);
    }

    /// Bump `last_used_at` on the entry whose canonical path matches `path`.
    /// Persists the registry on success. Silent no-op if the path isn't registered.
    pub fn touch_path(path: &Path) -> anyhow::Result<()> {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let mut reg = Self::load()?;
        let now = chrono::Utc::now().to_rfc3339();
        let mut changed = false;
        for r in &mut reg.repos {
            if r.path.canonicalize().ok().as_ref() == Some(&canonical) {
                r.last_used_at = Some(now.clone());
                changed = true;
                break;
            }
        }
        if changed {
            reg.save()?;
        }
        Ok(())
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
