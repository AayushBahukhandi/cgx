//! Best-effort detection of an Obsidian vault on the user's machine.
//!
//! No third-party probes — just reads well-known config files. If anything is
//! missing or malformed, returns `None`; callers prompt the user for a path instead.

use std::path::{Path, PathBuf};

/// Try to find a "primary" Obsidian vault, preferring whichever was opened most recently.
///
/// macOS: `~/Library/Application Support/obsidian/obsidian.json` lists registered vaults.
/// Falls back to a couple of common directory names if no config is found.
pub fn detect_default_vault() -> Option<PathBuf> {
    if let Some(vaults) = registered_vaults() {
        // Pick the vault with the highest `ts` (last-opened timestamp).
        let mut entries: Vec<&serde_json::Value> = vaults
            .as_object()?
            .values()
            .filter(|v| v.get("path").and_then(|p| p.as_str()).is_some())
            .collect();
        entries
            .sort_by_key(|v| std::cmp::Reverse(v.get("ts").and_then(|t| t.as_i64()).unwrap_or(0)));
        if let Some(first) = entries.first() {
            if let Some(path) = first.get("path").and_then(|p| p.as_str()) {
                let p = PathBuf::from(path);
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }

    fallback_paths().into_iter().find(|p| p.is_dir())
}

/// Returns every vault Obsidian knows about, oldest→newest (or `None` if config is missing).
pub fn list_registered_vaults() -> Vec<PathBuf> {
    let Some(vaults) = registered_vaults() else {
        return Vec::new();
    };
    let Some(obj) = vaults.as_object() else {
        return Vec::new();
    };
    let mut entries: Vec<&serde_json::Value> = obj.values().collect();
    entries.sort_by_key(|v| v.get("ts").and_then(|t| t.as_i64()).unwrap_or(0));
    entries
        .into_iter()
        .filter_map(|v| v.get("path").and_then(|p| p.as_str()).map(PathBuf::from))
        .filter(|p| p.exists())
        .collect()
}

fn registered_vaults() -> Option<serde_json::Value> {
    let config_path = obsidian_config_path()?;
    let content = std::fs::read_to_string(&config_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    json.get("vaults").cloned()
}

fn obsidian_config_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    if cfg!(target_os = "macos") {
        Some(
            home.join("Library")
                .join("Application Support")
                .join("obsidian")
                .join("obsidian.json"),
        )
    } else if cfg!(target_os = "linux") {
        Some(home.join(".config").join("obsidian").join("obsidian.json"))
    } else if cfg!(target_os = "windows") {
        std::env::var("APPDATA")
            .ok()
            .map(|appdata| Path::new(&appdata).join("obsidian").join("obsidian.json"))
    } else {
        None
    }
}

fn fallback_paths() -> Vec<PathBuf> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return Vec::new(),
    };
    vec![
        home.join("Documents").join("Obsidian Vault"),
        home.join("Obsidian"),
        home.join("Documents").join("Obsidian"),
    ]
}
