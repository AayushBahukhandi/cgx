//! Helper for `cgx docs prompts` — scan the vault for module notes whose
//! `<!-- cgx-prompt -->` block hasn't been filled in yet.
//!
//! A "filled" note is one where the cgx-prompt markers have been replaced
//! with prose (i.e., the markers no longer appear in the file).

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct PromptEntry {
    pub note_path: PathBuf,
    pub source_path: Option<String>,
    pub byte_offset_begin: usize,
    pub byte_offset_end: usize,
    pub packet: String,
}

/// Walk the vault and return every note that still contains an unfilled packet.
pub fn list_unfilled(vault_root: &Path) -> std::io::Result<Vec<PromptEntry>> {
    let mut out = Vec::new();
    walk(vault_root, &mut out)?;
    Ok(out)
}

fn walk(dir: &Path, out: &mut Vec<PromptEntry>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            // Skip dot-folders (.obsidian, .git, etc.)
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with('.'))
                .unwrap_or(false)
            {
                continue;
            }
            walk(&path, out)?;
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if let Some((b, e, packet, source)) = find_packet(&content) {
            out.push(PromptEntry {
                note_path: path.clone(),
                source_path: source,
                byte_offset_begin: b,
                byte_offset_end: e,
                packet,
            });
        }
    }
    Ok(())
}

const BEGIN: &str = "<!-- cgx-prompt:begin -->";
const END: &str = "<!-- cgx-prompt:end -->";

fn find_packet(content: &str) -> Option<(usize, usize, String, Option<String>)> {
    let begin = content.find(BEGIN)?;
    let end_marker = content[begin..].find(END)? + begin;
    let end = end_marker + END.len();
    let packet = content[begin..end].to_string();
    let source_path = find_frontmatter_field(content, "path");
    Some((begin, end, packet, source_path))
}

fn find_frontmatter_field(content: &str, field: &str) -> Option<String> {
    if !content.starts_with("---") {
        return None;
    }
    let after = &content[3..];
    let end = after.find("\n---")?;
    let fm = &after[..end];
    let prefix = format!("{}: ", field);
    for line in fm.lines() {
        if let Some(rest) = line.trim_start().strip_prefix(&prefix) {
            let rest = rest.trim();
            let rest = rest.trim_matches('"').trim_matches('\'');
            return Some(rest.to_string());
        }
    }
    None
}

/// Replace an unfilled packet in `content` with the given prose.
/// Returns the new full file content, or None if no packet was found.
pub fn fill_packet(content: &str, prose: &str) -> Option<String> {
    let (b, e, _, _) = find_packet(content)?;
    let mut out = String::with_capacity(content.len() + prose.len());
    out.push_str(&content[..b]);
    out.push_str(prose.trim_end());
    out.push('\n');
    out.push_str(&content[e..]);
    Some(out)
}
