//! Wiki-link / markdown-link formatting + safe filename conversion.

use super::WikiLinkStyle;

/// Format a link to a note path (without `.md`) with a display alias.
///
/// Obsidian renders `[[path|alias]]`; markdown viewers render `[alias](path.md)`.
pub fn format_link(target: &str, alias: &str, style: WikiLinkStyle) -> String {
    match style {
        WikiLinkStyle::Obsidian => format!("[[{}|{}]]", target, alias),
        WikiLinkStyle::Markdown => format!("[{}]({}.md)", alias, target),
    }
}

/// Convert an arbitrary string (community label, file path, symbol name) into a
/// filesystem-safe note name. Preserves directory separators in paths but replaces
/// other unsafe chars with `-`.
pub fn safe_filename(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut last_dash = false;
    for c in raw.chars() {
        match c {
            '/' | '\\' => {
                out.push('/');
                last_dash = false;
            }
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '.' => {
                out.push(c);
                last_dash = false;
            }
            '-' => {
                if !last_dash {
                    out.push('-');
                    last_dash = true;
                }
            }
            _ => {
                if !last_dash {
                    out.push('-');
                    last_dash = true;
                }
            }
        }
    }
    out.trim_matches('-').trim_start_matches('/').to_string()
}

/// Build the canonical module-note path (without `.md`) for a source file.
pub fn module_note_target(community_slug: &str, file_path: &str) -> String {
    format!("30-Modules/{}/{}", community_slug, safe_filename(file_path))
}

/// Group a source file by its meaningful containing directory.
///
/// Examples:
/// - `crates/cgx-engine/src/graph.rs`           → `cgx-engine`
/// - `crates/cgx-engine/src/parsers/rust.rs`    → `cgx-engine/parsers`
/// - `packages/web-ui/src/components/App.tsx`   → `web-ui/components`
/// - `tests/integration/auth.test.ts`           → `tests/integration`
/// - `README.md`                                → `root`
pub fn file_group(file_path: &str) -> String {
    let parent = file_path.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
    if parent.is_empty() {
        return "root".to_string();
    }
    // Drop noisy roots and then collapse `src` segments.
    let trimmed = parent
        .trim_start_matches("./")
        .trim_start_matches("crates/")
        .trim_start_matches("packages/")
        .trim_start_matches("apps/")
        .trim_start_matches("services/");
    let parts: Vec<&str> = trimmed
        .split('/')
        .filter(|s| !s.is_empty() && *s != "src")
        .collect();
    if parts.is_empty() {
        return "root".to_string();
    }
    // Keep at most 2 segments so paths stay readable.
    let take = parts.len().min(2);
    let joined: String = parts[..take].join("/");
    safe_filename(&joined)
}

/// Slug form of [`file_group`] suitable as a single path segment.
pub fn file_group_slug(file_path: &str) -> String {
    let g = file_group(file_path);
    g.replace('/', "-")
}

/// Build the canonical Public-API note path for a community.
pub fn public_api_target(community_slug: &str) -> String {
    format!("10-PublicAPI/{}", community_slug)
}

/// Slug a community label for use as a folder/filename. Slashes are flattened to `-`
/// since this is a single path segment, not a tree.
pub fn community_slug(label: &str, id: i64) -> String {
    let slug = safe_filename(label).to_lowercase().replace('/', "-");
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        format!("community-{}", id)
    } else {
        slug
    }
}
