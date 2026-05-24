//! Per-file slice-hash tracking, persisted to `~/.cgx/<repo_id>/docs_state.json`.
//!
//! A "slice hash" is a stable digest of just the parts of the graph that drive
//! a single file's module note: the symbol IDs and line ranges, in/out edge counts,
//! community membership, complexity bucket, and top owners. If two runs produce
//! the same slice hash for a file, its note is byte-identical and can be skipped.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::graph::FileSummary;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DocsState {
    pub generated_at: String,
    pub files: HashMap<String, FileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub slice_hash: String,
    pub last_gen_at: String,
}

/// Compute the slice hash for a single file from its [`FileSummary`].
pub fn slice_hash(summary: &FileSummary) -> String {
    let mut hasher = Sha256::new();
    hasher.update(summary.path.as_bytes());
    hasher.update(summary.community.to_be_bytes());

    let mut symbol_ids: Vec<(String, u32, u32)> = summary
        .symbols
        .iter()
        .map(|n| (n.id.clone(), n.line_start, n.line_end))
        .collect();
    symbol_ids.sort();
    for (id, ls, le) in &symbol_ids {
        hasher.update(id.as_bytes());
        hasher.update(ls.to_be_bytes());
        hasher.update(le.to_be_bytes());
    }

    hasher.update(summary.callers.len().to_be_bytes());
    hasher.update(summary.callees.len().to_be_bytes());
    hasher.update(summary.tests.len().to_be_bytes());

    // Bucket complexity into deciles so trivial drift doesn't invalidate the hash.
    let complexity_bucket = (summary.complexity / 10.0).floor() as i64;
    hasher.update(complexity_bucket.to_be_bytes());

    let mut owner_ids: Vec<&String> = summary.owners.iter().map(|(n, _)| n).collect();
    owner_ids.sort();
    owner_ids.truncate(3);
    for o in owner_ids {
        hasher.update(o.as_bytes());
    }

    format!("{:x}", hasher.finalize())
}

/// Read the persisted state file, returning a default if missing.
pub fn load_state(repo_id: &str) -> DocsState {
    match state_path(repo_id) {
        Some(p) => std::fs::read_to_string(&p)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default(),
        None => DocsState::default(),
    }
}

/// Persist the state file (creates parent dirs as needed).
pub fn save_state(repo_id: &str, state: &DocsState) -> anyhow::Result<()> {
    let Some(path) = state_path(repo_id) else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(state)?;
    std::fs::write(&path, json)?;
    Ok(())
}

fn state_path(repo_id: &str) -> Option<PathBuf> {
    Some(
        dirs::home_dir()?
            .join(".cgx")
            .join(repo_id)
            .join("docs_state.json"),
    )
}

/// Convenience: build a fresh entry for the current time.
pub fn entry_now(slice_hash: String) -> FileEntry {
    FileEntry {
        slice_hash,
        last_gen_at: chrono::Utc::now().to_rfc3339(),
    }
}

/// Whether a file should be regenerated in incremental mode.
pub fn needs_regen(state: &DocsState, file_path: &Path, new_hash: &str) -> bool {
    match state.files.get(&file_path.to_string_lossy().to_string()) {
        Some(entry) => entry.slice_hash != new_hash,
        None => true,
    }
}
