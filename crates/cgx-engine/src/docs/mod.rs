//! Layered, Obsidian-ready documentation vault generator.
//!
//! The docs generator turns the cgx graph into a markdown vault organised by the
//! Diátaxis-ish layout described in `cgx-docs`:
//!
//! - `00-Overview/` — Architecture.md, Glossary.md
//! - `10-PublicAPI/` — one note per community's exported surface
//! - `20-Architecture/` — Communities, CrossClusterDeps, EntryPoints
//! - `30-Modules/<community>/<file>.md` — per-file notes with prompt packets
//! - `40-Risk/` — Hotspots, ComplexityHigh, DeadCode, Duplicates
//! - `50-Ownership/` — Owners, BlameGraph
//!
//! cgx never calls an LLM. Each module note ends with a `<!-- cgx-prompt -->` block
//! that contains all context the user's AI agent needs to produce prose without
//! re-exploring the repo. The agent runs *outside* cgx.

pub mod incremental;
pub mod label;
pub mod layered;
pub mod obsidian_detect;
pub mod project;
pub mod prompt_packet;
pub mod prompts_index;
pub mod role;
pub mod wiki_link;

use std::path::{Path, PathBuf};

use crate::graph::GraphDb;

/// User-selected output target for `cgx docs`.
#[derive(Debug, Clone)]
pub enum DocsTarget {
    /// Write to a path inside the working repo (e.g. `./cgx-docs/`).
    Local(PathBuf),
    /// Write to an Obsidian vault. If the inner path is empty, auto-detect.
    Vault(Option<PathBuf>),
}

/// Mode of generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocsMode {
    /// Re-write every note from scratch.
    Full,
    /// Only re-write module notes whose graph slice changed since the last run.
    /// Top-level index notes (Architecture, Hotspots, ...) are always re-written.
    Incremental,
}

/// Options that flow from CLI flags / `DocsConfig` into the generator.
#[derive(Debug, Clone)]
pub struct DocsOptions {
    pub prompt_packets: bool,
    pub frontmatter: bool,
    pub wiki_links_style: WikiLinkStyle,
    pub include_dead_code: bool,
    pub include_duplicates: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WikiLinkStyle {
    /// `[[path/to/note|alias]]` (Obsidian).
    Obsidian,
    /// `[alias](path/to/note.md)` (plain markdown).
    Markdown,
}

impl Default for DocsOptions {
    fn default() -> Self {
        Self {
            prompt_packets: true,
            frontmatter: true,
            wiki_links_style: WikiLinkStyle::Obsidian,
            include_dead_code: true,
            include_duplicates: true,
        }
    }
}

/// Stats returned by [`generate_vault`].
#[derive(Debug, Clone, Default)]
pub struct DocsReport {
    pub output_dir: PathBuf,
    pub module_notes_written: usize,
    pub module_notes_skipped: usize,
    pub index_notes_written: usize,
    pub mode: &'static str,
}

/// Main entry point — generate (or refresh) a docs vault for the repo indexed in `db`.
pub fn generate_vault(
    repo_path: &Path,
    db: &GraphDb,
    target: DocsTarget,
    mode: DocsMode,
    opts: &DocsOptions,
) -> anyhow::Result<DocsReport> {
    let output_dir = match target {
        DocsTarget::Local(p) => p,
        DocsTarget::Vault(Some(p)) => p.join("cgx-docs"),
        DocsTarget::Vault(None) => obsidian_detect::detect_default_vault()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No Obsidian vault detected. Pass a path: `cgx docs --vault <path>`."
                )
            })?
            .join("cgx-docs"),
    };

    layered::write_vault(repo_path, db, &output_dir, mode, opts)
}
