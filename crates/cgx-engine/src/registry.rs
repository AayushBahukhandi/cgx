use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoEntry {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub db_path: PathBuf,
    pub indexed_at: String,
    pub node_count: u64,
    pub edge_count: u64,
    #[serde(default)]
    pub language_breakdown: HashMap<String, f64>,
}

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

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn register(&mut self, entry: RepoEntry) {
        self.repos.retain(|r| r.id != entry.id);
        self.repos.push(entry);
    }

    pub fn find_by_path(&self, path: &Path) -> Option<&RepoEntry> {
        let canonical = path.canonicalize().ok()?;
        self.repos
            .iter()
            .find(|r| r.path.canonicalize().ok().as_ref() == Some(&canonical))
    }

    pub fn find_by_id(&self, id: &str) -> Option<&RepoEntry> {
        self.repos.iter().find(|r| r.id == id)
    }
}
