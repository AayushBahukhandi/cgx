//! Diátaxis-ish layered vault writer.
//!
//! Top-level indices (00-Overview .. 50-Ownership) are always re-written.
//! Module notes (30-Modules/...) honour the incremental slice-hash gate.
//!
//! Layout:
//! ```text
//! cgx-docs/
//!   .obsidian/
//!   README.md                     ← entry point with "Start here"
//!   00-Overview/
//!     Architecture.md             ← project, stack, languages, top modules
//!     HowToNavigate.md            ← reading path for new readers
//!     Glossary.md                 ← node kinds, communities
//!   10-PublicAPI/<community>.md
//!   20-Architecture/
//!     Communities.md
//!     CrossClusterDeps.md
//!     EntryPoints.md
//!   30-Modules/<community>/<basename>.md  ← per-file notes (flattened)
//!   40-Risk/
//!     Hotspots.md
//!     ComplexityHigh.md
//!     DeadCode.md
//!     Duplicates.md
//!   50-Ownership/
//!     Owners.md
//!     BlameGraph.md
//! ```

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::graph::{
    ApiScope, CrossClusterEdge, EntryPoint, FileSummary, GraphDb, Node, PublicSymbol,
};

use super::incremental::{self, DocsState};
use super::label::build_community_labels;
use super::project::{detect as detect_project, DepKind, Dependency, ProjectInfo};
use super::prompt_packet;
use super::role::{classify as classify_role, FileRole};
use super::wiki_link::{community_slug, file_group, format_link, public_api_target, safe_filename};
use super::{DocsMode, DocsOptions, DocsReport, WikiLinkStyle};

pub fn write_vault(
    repo_path: &Path,
    db: &GraphDb,
    output_dir: &Path,
    mode: DocsMode,
    opts: &DocsOptions,
) -> anyhow::Result<DocsReport> {
    std::fs::create_dir_all(output_dir)?;
    std::fs::create_dir_all(output_dir.join(".obsidian"))?;

    // ---- Gather once, reuse everywhere ----
    let all_nodes = db.get_all_nodes()?;
    let communities = db.get_communities()?;
    let cgx_labels: HashMap<i64, String> = communities
        .iter()
        .map(|(id, label, _, _)| (*id, label.clone()))
        .collect();
    // Override with our better path-prefix-derived labels.
    let community_labels = build_community_labels(&all_nodes, &cgx_labels);
    let project = detect_project(repo_path);

    let mut file_paths: Vec<String> = all_nodes
        .iter()
        .filter(|n| n.kind == "File")
        .map(|n| n.path.clone())
        .collect();
    file_paths.sort();
    file_paths.dedup();

    // Precompute summaries + roles so indices can reference them too.
    let mut summaries: Vec<(String, FileSummary, FileRole)> = Vec::with_capacity(file_paths.len());
    for path in &file_paths {
        let Ok(summary) = db.get_file_summary(path) else {
            continue;
        };
        let role = classify_role(&summary);
        summaries.push((path.clone(), summary, role));
    }

    // ---- Incremental state ----
    let mut state = if mode == DocsMode::Incremental {
        incremental::load_state(&db.repo_id)
    } else {
        DocsState::default()
    };

    let mut module_written = 0usize;
    let mut module_skipped = 0usize;
    let mut index_written = 0usize;

    // ---- 30-Modules: one note per source file, grouped by file_group ----
    for (path, summary, role) in &summaries {
        let new_hash = incremental::slice_hash(summary);
        if mode == DocsMode::Incremental
            && !incremental::needs_regen(&state, Path::new(path), &new_hash)
        {
            module_skipped += 1;
            continue;
        }

        let group = file_group(path);
        let group_dir = group.replace('/', std::path::MAIN_SEPARATOR_STR);
        let basename = path.rsplit('/').next().unwrap_or(path);
        // Disambiguate basename collisions within the same group.
        let mut filename = safe_filename(basename);
        if collision_risk_in_group(basename, &group, &summaries) {
            let hash = short_hash(path);
            filename = format!("{}.{}", filename, hash);
        }
        let abs = output_dir
            .join("30-Modules")
            .join(&group_dir)
            .join(format!("{}.md", filename));
        if let Some(parent) = abs.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = render_module_note(db, summary, *role, &group, opts, &community_labels);
        std::fs::write(&abs, content)?;
        module_written += 1;

        state
            .files
            .insert(path.clone(), incremental::entry_now(new_hash));
    }

    // ---- Top-level indices: always rewritten ----
    index_written += write_overview(
        output_dir,
        db,
        &all_nodes,
        &project,
        &summaries,
        &community_labels,
        opts,
    )?;
    index_written += write_how_to_navigate(output_dir, &project, opts)?;
    index_written += write_glossary(output_dir, &all_nodes, &community_labels, opts)?;
    index_written += write_public_api(output_dir, &summaries, opts)?;
    index_written += write_architecture(output_dir, db, &summaries, &community_labels, opts)?;
    index_written += write_risk(output_dir, db, &all_nodes, opts)?;
    index_written += write_ownership(output_dir, db, &all_nodes, opts)?;
    index_written += write_readme(output_dir, db, &file_paths, &project, &summaries, opts)?;

    state.generated_at = chrono::Utc::now().to_rfc3339();
    incremental::save_state(&db.repo_id, &state)?;

    let _ = repo_path;
    Ok(DocsReport {
        output_dir: PathBuf::from(output_dir),
        module_notes_written: module_written,
        module_notes_skipped: module_skipped,
        index_notes_written: index_written,
        mode: match mode {
            DocsMode::Full => "full",
            DocsMode::Incremental => "incremental",
        },
    })
}

fn short_hash(s: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    let digest = format!("{:x}", h.finalize());
    digest[..6].to_string()
}

fn collision_risk_in_group(
    basename: &str,
    group: &str,
    summaries: &[(String, FileSummary, FileRole)],
) -> bool {
    summaries
        .iter()
        .filter(|(p, _, _)| file_group(p) == group && p.rsplit('/').next().unwrap_or(p) == basename)
        .count()
        > 1
}

// =====================================================================
// Module note (30-Modules/<community>/<basename>.md)
// =====================================================================

fn render_module_note(
    db: &GraphDb,
    summary: &FileSummary,
    role: FileRole,
    file_group_str: &str,
    opts: &DocsOptions,
    community_labels: &HashMap<i64, String>,
) -> String {
    let mut out = String::new();
    let community_label = community_labels
        .get(&summary.community)
        .cloned()
        .unwrap_or_else(|| format!("community-{}", summary.community));

    if opts.frontmatter {
        let unique_owners = dedup_keep_order(summary.owners.iter().map(|(n, _)| n.clone()));
        let _ = writeln!(out, "---");
        let _ = writeln!(out, "cgx_kind: module");
        let _ = writeln!(out, "role: {}", role.label());
        let _ = writeln!(out, "path: {}", yaml_quote(&summary.path));
        let _ = writeln!(
            out,
            "language: {}",
            if summary.language.is_empty() {
                "unknown"
            } else {
                &summary.language
            }
        );
        let _ = writeln!(out, "community: {}", yaml_quote(&community_label));
        if !unique_owners.is_empty() {
            let top: Vec<String> = unique_owners
                .iter()
                .take(3)
                .map(|n| yaml_quote(n))
                .collect();
            let _ = writeln!(out, "owners: [{}]", top.join(", "));
        }
        let _ = writeln!(out, "churn: {:.2}", summary.churn);
        let _ = writeln!(out, "complexity: {:.1}", summary.complexity);
        let _ = writeln!(out, "exported_count: {}", summary.exported_count);
        let _ = writeln!(out, "tags: [module, cgx, {}]", role.label());
        let _ = writeln!(out, "---");
        out.push('\n');
    }

    let basename = summary.path.rsplit('/').next().unwrap_or(&summary.path);
    let _ = writeln!(out, "# `{}`\n", basename);
    let _ = writeln!(out, "**Path:** `{}`  ", summary.path);
    let _ = writeln!(out, "**Group:** `{}`  ", file_group_str);
    let _ = writeln!(out, "**Community:** {}\n", community_label);

    // --- TL;DR ----------------------------------------------------------
    out.push_str(&tldr_line(summary, role));
    out.push_str("\n\n");

    // --- Existing prose docstrings — top of note, before tables ----------
    let existing_docs: Vec<(String, String)> = summary
        .symbols
        .iter()
        .filter_map(|sym| match db.get_doc_comment(&sym.id).ok().flatten() {
            Some(doc) if !doc.trim().is_empty() => Some((sym.name.clone(), doc)),
            _ => None,
        })
        .collect();

    if !existing_docs.is_empty() {
        out.push_str("## What's documented in source\n\n");
        for (name, doc) in &existing_docs {
            let _ = writeln!(out, "**`{}`** — {}\n", name, first_line(doc));
        }
        out.push('\n');
    } else {
        out.push_str("> _No symbol-level docstrings in source yet. The AI prose stub at the bottom of this note is the recommended next step._\n\n");
    }

    // --- Structure: symbols table with inline descriptions ---------------
    out.push_str("## Structure\n\n");
    if summary.symbols.is_empty() {
        out.push_str("_(no symbols extracted for this file)_\n\n");
    } else {
        out.push_str("| Kind | Name | Lines | Exported | Description |\n");
        out.push_str("|------|------|-------|----------|-------------|\n");
        let doc_map: HashMap<&str, &str> = existing_docs
            .iter()
            .map(|(n, d)| (n.as_str(), d.as_str()))
            .collect();
        for n in &summary.symbols {
            let desc = doc_map
                .get(n.name.as_str())
                .map(|d| first_line(d))
                .unwrap_or_default();
            let _ = writeln!(
                out,
                "| {} | `{}` | {}–{} | {} | {} |",
                n.kind,
                n.name,
                n.line_start,
                n.line_end,
                if n.exported { "✓" } else { "" },
                escape_table_cell(&desc),
            );
        }
        out.push('\n');
    }

    // --- Callers / Callees ---
    if !summary.callers.is_empty() {
        out.push_str("## Called by\n\n");
        for n in &summary.callers {
            let target = module_target_for(&n.path, &n.community, community_labels);
            let _ = writeln!(
                out,
                "- {} — `{}`",
                format_link(&target, &n.name, opts.wiki_links_style),
                n.path,
            );
        }
        out.push('\n');
    }
    if !summary.callees.is_empty() {
        out.push_str("## Calls into\n\n");
        for n in &summary.callees {
            let target = module_target_for(&n.path, &n.community, community_labels);
            let _ = writeln!(
                out,
                "- {} — `{}`",
                format_link(&target, &n.name, opts.wiki_links_style),
                n.path,
            );
        }
        out.push('\n');
    }

    // --- Tests ---
    if !summary.tests.is_empty() {
        out.push_str("## Tests covering this file\n\n");
        let paths: std::collections::BTreeSet<&str> =
            summary.tests.iter().map(|t| t.path.as_str()).collect();
        let count = summary.tests.len();
        let _ = writeln!(out, "{} test fn(s) across:", count);
        for p in paths.iter().copied().take(10) {
            let _ = writeln!(out, "- `{}`", p);
        }
        if paths.len() > 10 {
            let _ = writeln!(out, "- … {} more", paths.len() - 10);
        }
        out.push('\n');
    }

    // --- Ownership ---
    if !summary.owners.is_empty() {
        out.push_str("## Ownership\n\n");
        let mut seen = std::collections::HashSet::new();
        for (name, weight) in &summary.owners {
            if !seen.insert(name.clone()) {
                continue;
            }
            let _ = writeln!(out, "- {} (weight: {:.2})", name, weight);
        }
        out.push('\n');
    }

    // --- Prompt packet ---
    if opts.prompt_packets {
        out.push_str("## AI prose stub\n\n");
        out.push_str(
            "_Replace the block below with prose by feeding it to Claude/Cursor. Run `cgx docs prompts --next` to step through unfilled packets._\n\n",
        );
        out.push_str(&prompt_packet::build(db, summary));
        out.push('\n');
    }

    out
}

/// Build the canonical wiki-link target for a given file's module note.
fn module_target_for(path: &str, _community: &i64, _labels: &HashMap<i64, String>) -> String {
    let group = file_group(path);
    let basename = path.rsplit('/').next().unwrap_or(path);
    let filename = safe_filename(basename);
    format!("30-Modules/{}/{}", group, filename)
}

fn tldr_line(summary: &FileSummary, role: FileRole) -> String {
    let lang = if summary.language.is_empty() {
        "Source"
    } else {
        match summary.language.as_str() {
            "rust" => "Rust",
            "typescript" => "TypeScript",
            "javascript" => "JavaScript",
            "python" => "Python",
            "go" => "Go",
            "java" => "Java",
            "php" => "PHP",
            other => other,
        }
    };
    let role_desc = role.description();
    let exported = if summary.exported_count > 0 {
        format!(
            "{} exported symbol{}",
            summary.exported_count,
            if summary.exported_count == 1 { "" } else { "s" }
        )
    } else {
        "internal-only".to_string()
    };
    let edges = match (summary.callers.len(), summary.callees.len()) {
        (0, 0) => String::from("no cross-file edges"),
        (c, 0) => format!("called by {} other file(s)", c),
        (0, d) => format!("calls into {} other file(s)", d),
        (c, d) => format!("called by {} · calls into {}", c, d),
    };
    let risk = if summary.churn >= 0.5 || summary.complexity >= 10.0 {
        " · ⚠ hotspot".to_string()
    } else {
        String::new()
    };
    format!(
        "> **{} · {}** — {}, {}.{}",
        lang, role_desc, exported, edges, risk
    )
}

// =====================================================================
// 00-Overview/
// =====================================================================

#[allow(clippy::too_many_arguments)]
fn write_overview(
    output_dir: &Path,
    db: &GraphDb,
    all_nodes: &[Node],
    project: &ProjectInfo,
    summaries: &[(String, FileSummary, FileRole)],
    community_labels: &HashMap<i64, String>,
    opts: &DocsOptions,
) -> anyhow::Result<usize> {
    let dir = output_dir.join("00-Overview");
    std::fs::create_dir_all(&dir)?;

    let lang = db.get_language_breakdown().unwrap_or_default();
    let mut lang_entries: Vec<_> = lang.iter().collect();
    lang_entries.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut arch = String::new();
    if opts.frontmatter {
        arch.push_str("---\ncgx_kind: overview\ntags: [overview, architecture, cgx]\n---\n\n");
    }

    let _ = writeln!(arch, "# Architecture overview\n");

    // ---- Project header ----
    if let Some(name) = &project.name {
        if let Some(version) = &project.version {
            let _ = writeln!(arch, "**Project:** `{}` v{}", name, version);
        } else {
            let _ = writeln!(arch, "**Project:** `{}`", name);
        }
    }
    if let Some(desc) = &project.description {
        let _ = writeln!(arch, "\n> {}\n", desc);
    }
    if let Some(rd) = &project.readme_excerpt {
        let _ = writeln!(arch, "{}\n", rd);
    }

    // ---- Stack ----
    if !project.stack.is_empty() {
        let _ = writeln!(arch, "**Stack:** {}", project.stack.join(", "));
    }
    if !lang_entries.is_empty() {
        let langs: Vec<String> = lang_entries
            .iter()
            .map(|(l, p)| format!("{} {:.0}%", l, **p * 100.0))
            .collect();
        let _ = writeln!(arch, "**Languages by node count:** {}", langs.join(", "));
    }
    let _ = writeln!(
        arch,
        "**Manifests detected:** {}",
        project.manifests.join(", ")
    );
    let _ = writeln!(
        arch,
        "**Graph size:** {} nodes · {} edges · {} files · {} communities\n",
        db.node_count().unwrap_or(0),
        db.edge_count().unwrap_or(0),
        summaries.len(),
        community_labels.len()
    );

    // ---- Dependencies (with purpose + usage check) ----
    if !project.deps.is_empty() {
        let runtime: Vec<&Dependency> = project
            .deps
            .iter()
            .filter(|d| matches!(d.kind, DepKind::Runtime))
            .collect();
        let dev: Vec<&Dependency> = project
            .deps
            .iter()
            .filter(|d| matches!(d.kind, DepKind::Dev | DepKind::Build | DepKind::Peer))
            .collect();
        let unused: Vec<&Dependency> = project
            .deps
            .iter()
            .filter(|d| !d.used && matches!(d.kind, DepKind::Runtime))
            .collect();

        arch.push_str("## Dependencies and what they're used for\n\n");
        let _ = writeln!(
            arch,
            "{} declared in total ({} runtime · {} dev/build/peer). {} runtime dep(s) have no detected import in source.\n",
            project.deps.len(),
            runtime.len(),
            dev.len(),
            unused.len()
        );

        if !runtime.is_empty() {
            arch.push_str("### Runtime\n\n");
            arch.push_str("| Package | Used for | Files importing | Status |\n|---|---|---|---|\n");
            for d in &runtime {
                let status = if d.used {
                    format!("✓ {}", d.use_count)
                } else {
                    "⚠ unused?".to_string()
                };
                let _ = writeln!(
                    arch,
                    "| `{}` | {} | {} | {} |",
                    d.name,
                    escape_table_cell(&d.purpose),
                    d.use_count,
                    status
                );
            }
            arch.push('\n');
        }

        if !dev.is_empty() {
            arch.push_str("### Dev / build / peer\n\n");
            arch.push_str("| Package | Used for | Kind |\n|---|---|---|\n");
            for d in &dev {
                let _ = writeln!(
                    arch,
                    "| `{}` | {} | {} |",
                    d.name,
                    escape_table_cell(&d.purpose),
                    d.kind.label(),
                );
            }
            arch.push('\n');
        }

        if !unused.is_empty() {
            arch.push_str("> ⚠ **Possibly unused runtime dependencies:** ");
            let names: Vec<String> = unused.iter().map(|d| format!("`{}`", d.name)).collect();
            arch.push_str(&names.join(", "));
            arch.push_str(". cgx scanned every source file for `use`/`import` statements but found none referencing these. Confirm before removing — they may be loaded via dynamic dispatch, build scripts, or feature flags.\n\n");
        }
    }

    // ---- Role breakdown ----
    let role_counts = role_distribution(summaries);
    if !role_counts.is_empty() {
        arch.push_str("## Files by role\n\n");
        arch.push_str("| Role | Count |\n|---|---|\n");
        let mut ordered: Vec<_> = role_counts.iter().collect();
        ordered.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
        for (role, count) in ordered {
            let _ = writeln!(arch, "| {} | {} |", role, count);
        }
        arch.push('\n');
    }

    // ---- Top groups (largest source directories) ----
    arch.push_str("## Largest groups\n\n");
    let group_sizes: Vec<(String, usize, usize)> = {
        let mut counts: HashMap<String, (usize, usize)> = HashMap::new();
        for (_, s, _) in summaries {
            let entry = counts.entry(file_group(&s.path)).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += s.exported_count;
        }
        let mut v: Vec<(String, usize, usize)> =
            counts.into_iter().map(|(g, (f, e))| (g, f, e)).collect();
        v.sort_by_key(|(_, files, _)| std::cmp::Reverse(*files));
        v
    };
    for (group, files, exported) in group_sizes.iter().take(10) {
        let slug = safe_filename(group).replace('/', "-");
        let link = format_link(&public_api_target(&slug), group, opts.wiki_links_style);
        let _ = writeln!(
            arch,
            "- {} — {} file(s), {} exported symbol(s)",
            link, files, exported
        );
    }
    arch.push('\n');
    let _ = all_nodes; // retained for signature compat; not used anymore here.

    // ---- Entry points (program roots) ----
    let entry_files: Vec<&(String, FileSummary, FileRole)> = summaries
        .iter()
        .filter(|(_, _, r)| matches!(r, FileRole::Entry))
        .collect();
    if !entry_files.is_empty() {
        arch.push_str("## Entry points (program roots)\n\n");
        for (path, summary, _) in &entry_files {
            let target = module_target_for(path, &summary.community, community_labels);
            let _ = writeln!(
                arch,
                "- {} — `{}`",
                format_link(
                    &target,
                    path.rsplit('/').next().unwrap_or(path),
                    opts.wiki_links_style
                ),
                path
            );
        }
        arch.push('\n');
    }

    // ---- See also ----
    arch.push_str("## See also\n\n");
    let _ = writeln!(
        arch,
        "- {}",
        format_link(
            "00-Overview/HowToNavigate",
            "How to navigate",
            opts.wiki_links_style
        )
    );
    let _ = writeln!(
        arch,
        "- {}",
        format_link("00-Overview/Glossary", "Glossary", opts.wiki_links_style)
    );
    let _ = writeln!(
        arch,
        "- {}",
        format_link(
            "20-Architecture/Groups",
            "Source groups",
            opts.wiki_links_style
        )
    );
    let _ = writeln!(
        arch,
        "- {}",
        format_link(
            "20-Architecture/EntryPoints",
            "Entry points (graph roots)",
            opts.wiki_links_style
        )
    );
    let _ = writeln!(
        arch,
        "- {}",
        format_link("40-Risk/Hotspots", "Hotspots", opts.wiki_links_style)
    );

    std::fs::write(dir.join("Architecture.md"), arch)?;
    Ok(1)
}

fn write_how_to_navigate(
    output_dir: &Path,
    project: &ProjectInfo,
    opts: &DocsOptions,
) -> anyhow::Result<usize> {
    let dir = output_dir.join("00-Overview");
    std::fs::create_dir_all(&dir)?;

    let mut out = String::new();
    if opts.frontmatter {
        out.push_str("---\ncgx_kind: how_to_navigate\ntags: [overview, cgx]\n---\n\n");
    }
    out.push_str("# How to navigate this vault\n\n");

    if let Some(name) = &project.name {
        let _ = writeln!(
            out,
            "This vault documents **`{}`**. Read these in order if you're new.\n",
            name
        );
    } else {
        out.push_str("Read these in order if you're new to this codebase.\n\n");
    }

    let _ = writeln!(
        out,
        "1. {} — what the project is, stack, top-level shape.",
        format_link(
            "00-Overview/Architecture",
            "Architecture overview",
            opts.wiki_links_style
        )
    );
    let _ = writeln!(
        out,
        "2. {} — directory-based modules, with file counts and exported-symbol counts.",
        format_link(
            "20-Architecture/Groups",
            "Source groups",
            opts.wiki_links_style
        )
    );
    let _ = writeln!(
        out,
        "3. {} — exported API of each group.",
        format_link("10-PublicAPI", "Public APIs", opts.wiki_links_style)
    );
    let _ = writeln!(out, "4. `30-Modules/<group>/<file>.md` — per-file notes with structure tables, callers/callees, tests, and AI prose stubs.");
    let _ = writeln!(
        out,
        "5. {} — files most likely to break if you touch them.",
        format_link("40-Risk/Hotspots", "Hotspots", opts.wiki_links_style)
    );
    out.push('\n');

    out.push_str("## How module notes work\n\n");
    out.push_str("Each module note has:\n\n");
    out.push_str("- **TL;DR** — one line: language · role · symbol count · cross-file edges.\n");
    out.push_str("- **What's documented in source** — any docstrings the parser found.\n");
    out.push_str("- **Structure** — every symbol with its line range, export status, and inline description.\n");
    out.push_str("- **Called by / Calls into** — wiki-linked neighbours.\n");
    out.push_str("- **Tests / Ownership** — coverage and authorship.\n");
    out.push_str("- **AI prose stub** — a `<!-- cgx-prompt -->` block. Feed it to Claude / Cursor to generate prose without re-reading the source.\n");

    out.push_str("\n## Regenerating these docs\n\n");
    out.push_str("```bash\n");
    out.push_str("cgx analyze                                    # refresh the graph\n");
    out.push_str(
        "cgx docs generate --vault                      # full rebuild into Obsidian vault\n",
    );
    out.push_str(
        "cgx docs generate --vault --incremental        # only files whose graph slice changed\n",
    );
    out.push_str("cgx docs generate --vault --force              # alias for full rebuild\n");
    out.push_str("cgx docs prompts                               # list unfilled AI prose stubs\n");
    out.push_str("cgx docs prompts --next                        # print the next packet to send to your AI\n");
    out.push_str("```\n");

    std::fs::write(dir.join("HowToNavigate.md"), out)?;
    Ok(1)
}

fn write_glossary(
    output_dir: &Path,
    all_nodes: &[Node],
    community_labels: &HashMap<i64, String>,
    opts: &DocsOptions,
) -> anyhow::Result<usize> {
    let dir = output_dir.join("00-Overview");
    std::fs::create_dir_all(&dir)?;

    let mut out = String::new();
    if opts.frontmatter {
        out.push_str("---\ncgx_kind: glossary\ntags: [glossary, cgx]\n---\n\n");
    }
    out.push_str("# Glossary\n\n");
    out.push_str("## Node kinds\n\n");
    for kind in [
        "File", "Function", "Class", "Variable", "Type", "Module", "Author",
    ] {
        let count = all_nodes.iter().filter(|n| n.kind == kind).count();
        let _ = writeln!(out, "- **{}** — {} node(s)", kind, count);
    }
    out.push_str("\n## Communities (Louvain clusters)\n\n");
    let mut sizes: HashMap<i64, usize> = HashMap::new();
    for n in all_nodes {
        *sizes.entry(n.community).or_insert(0) += 1;
    }
    let mut ordered: Vec<(&i64, &usize)> = sizes.iter().collect();
    ordered.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
    for (id, count) in ordered {
        let label = community_labels
            .get(id)
            .cloned()
            .unwrap_or_else(|| format!("community-{}", id));
        let _ = writeln!(out, "- **{}** (id={}, {} nodes)", label, id, count);
    }
    std::fs::write(dir.join("Glossary.md"), out)?;
    Ok(1)
}

// =====================================================================
// 10-PublicAPI/
// =====================================================================

fn write_public_api(
    output_dir: &Path,
    summaries: &[(String, FileSummary, FileRole)],
    opts: &DocsOptions,
) -> anyhow::Result<usize> {
    let dir = output_dir.join("10-PublicAPI");
    std::fs::create_dir_all(&dir)?;

    // Bucket symbols by file group, not by Louvain community.
    let mut by_group: HashMap<String, Vec<&FileSummary>> = HashMap::new();
    for (_, summary, _) in summaries {
        by_group
            .entry(file_group(&summary.path))
            .or_default()
            .push(summary);
    }

    let mut written = 0usize;
    for (group, files) in &by_group {
        let exported_count: usize = files.iter().map(|s| s.exported_count).sum();
        if exported_count == 0 {
            continue;
        }
        let mut content = String::new();
        if opts.frontmatter {
            content.push_str("---\ncgx_kind: public_api\ntags: [public-api, cgx]\n---\n\n");
        }
        let _ = writeln!(content, "# Public API — `{}`\n", group);
        let _ = writeln!(
            content,
            "{} file(s) · {} exported symbol(s).\n",
            files.len(),
            exported_count
        );

        let mut sorted_files: Vec<&&FileSummary> = files.iter().collect();
        sorted_files.sort_by(|a, b| a.path.cmp(&b.path));
        for file in sorted_files {
            let basename = file.path.rsplit('/').next().unwrap_or(&file.path);
            let target = module_target_for(&file.path, &file.community, &HashMap::new());
            let _ = writeln!(
                content,
                "## {} ({} exported)\n",
                format_link(&target, basename, opts.wiki_links_style),
                file.exported_count
            );
            let _ = writeln!(content, "`{}`\n", file.path);
            for sym in &file.symbols {
                if !sym.exported {
                    continue;
                }
                let _ = writeln!(
                    content,
                    "- **`{}`** ({}) — line {}",
                    sym.name, sym.kind, sym.line_start
                );
            }
            content.push('\n');
        }
        let slug = safe_filename(group).replace('/', "-");
        std::fs::write(dir.join(format!("{}.md", slug)), content)?;
        written += 1;
    }
    Ok(written)
}

#[allow(dead_code)]
fn render_symbol_row_unused(out: &mut String, s: &PublicSymbol) {
    let _ = writeln!(out, "### `{}` ({})", s.name, s.kind);
    let _ = writeln!(out, "- File: `{}` (line {})", s.path, s.line_start);
    if let Some(doc) = &s.doc_comment {
        if !doc.trim().is_empty() {
            out.push_str("\n```\n");
            out.push_str(doc.trim());
            out.push_str("\n```\n\n");
        }
    } else {
        out.push('\n');
    }
}

// =====================================================================
// 20-Architecture/
// =====================================================================

fn write_architecture(
    output_dir: &Path,
    db: &GraphDb,
    summaries: &[(String, FileSummary, FileRole)],
    community_labels: &HashMap<i64, String>,
    opts: &DocsOptions,
) -> anyhow::Result<usize> {
    let dir = output_dir.join("20-Architecture");
    std::fs::create_dir_all(&dir)?;

    // Groups.md (replaces Communities.md as the primary navigation index)
    let mut by_group: HashMap<String, Vec<&FileSummary>> = HashMap::new();
    for (_, summary, _) in summaries {
        by_group
            .entry(file_group(&summary.path))
            .or_default()
            .push(summary);
    }
    let mut sorted_groups: Vec<(&String, &Vec<&FileSummary>)> = by_group.iter().collect();
    sorted_groups.sort_by_key(|(g, _)| (*g).clone());

    let mut groups = String::new();
    if opts.frontmatter {
        groups.push_str("---\ncgx_kind: groups\ntags: [architecture, cgx]\n---\n\n");
    }
    groups.push_str("# Source groups\n\n");
    groups.push_str(
        "Each group is a directory of related files. Click into one to see its public API.\n\n",
    );
    groups.push_str("| Group | Files | Exported symbols |\n|---|---|---|\n");
    for (group, files) in &sorted_groups {
        let exported: usize = files.iter().map(|s| s.exported_count).sum();
        let slug = safe_filename(group).replace('/', "-");
        let _ = writeln!(
            groups,
            "| {} | {} | {} |",
            format_link(&public_api_target(&slug), group, opts.wiki_links_style),
            files.len(),
            exported,
        );
    }
    std::fs::write(dir.join("Groups.md"), groups)?;

    // Communities.md kept as raw Louvain output for graph nerds.
    let mut comm = String::new();
    if opts.frontmatter {
        comm.push_str("---\ncgx_kind: communities\ntags: [architecture, cgx]\n---\n\n");
    }
    comm.push_str("# Louvain communities (raw graph clusters)\n\n");
    comm.push_str(
        "These are the raw clusters detected by Louvain modularity optimisation over the call graph. \
         For navigation, prefer [[20-Architecture/Groups|Source groups]] — it organises by directory.\n\n",
    );
    comm.push_str("| ID | Label | Public symbols |\n|---|---|---|\n");
    let mut entries: Vec<(&i64, &String)> = community_labels.iter().collect();
    entries.sort_by(|a, b| a.1.cmp(b.1));
    for (id, label) in entries {
        let public_count = db
            .get_public_api(ApiScope::Community(*id))
            .map(|v| v.len())
            .unwrap_or(0);
        let _ = writeln!(comm, "| {} | {} | {} |", id, label, public_count);
    }
    std::fs::write(dir.join("Communities.md"), comm)?;

    // CrossClusterDeps.md
    let edges = db.get_cross_cluster_deps().unwrap_or_default();
    let mut cross = String::new();
    if opts.frontmatter {
        cross.push_str("---\ncgx_kind: cross_cluster\ntags: [architecture, cgx]\n---\n\n");
    }
    cross.push_str("# Cross-cluster dependencies\n\n");
    if edges.is_empty() {
        cross.push_str("_(no cross-cluster edges detected)_\n");
    } else {
        cross.push_str("| From | → | To | Edges | Total weight |\n|---|---|---|---|---|\n");
        for e in edges.iter().take(50) {
            render_cross_edge(&mut cross, e, community_labels, opts.wiki_links_style);
        }
    }
    std::fs::write(dir.join("CrossClusterDeps.md"), cross)?;

    // EntryPoints.md
    let entries = db.list_entry_points().unwrap_or_default();
    let mut ep = String::new();
    if opts.frontmatter {
        ep.push_str("---\ncgx_kind: entry_points\ntags: [architecture, cgx]\n---\n\n");
    }
    ep.push_str("# Entry points\n\n");
    ep.push_str("Nodes with no inbound graph edges — likely public entry points, event handlers, or dead candidates.\n\n");
    if entries.is_empty() {
        ep.push_str("_(none detected)_\n");
    } else {
        render_entry_points(&mut ep, &entries);
    }
    std::fs::write(dir.join("EntryPoints.md"), ep)?;

    Ok(4)
}

fn render_cross_edge(
    out: &mut String,
    e: &CrossClusterEdge,
    labels: &HashMap<i64, String>,
    style: WikiLinkStyle,
) {
    let src_label = labels
        .get(&e.src_community)
        .cloned()
        .unwrap_or_else(|| format!("community-{}", e.src_community));
    let dst_label = labels
        .get(&e.dst_community)
        .cloned()
        .unwrap_or_else(|| format!("community-{}", e.dst_community));
    let src_slug = community_slug(&src_label, e.src_community);
    let dst_slug = community_slug(&dst_label, e.dst_community);
    let src_link = format_link(&public_api_target(&src_slug), &src_label, style);
    let dst_link = format_link(&public_api_target(&dst_slug), &dst_label, style);
    let _ = writeln!(
        out,
        "| {} | → | {} | {} | {:.2} |",
        src_link, dst_link, e.edge_count, e.total_weight
    );
}

fn render_entry_points(out: &mut String, entries: &[EntryPoint]) {
    let mut by_file: HashMap<String, Vec<&EntryPoint>> = HashMap::new();
    for ep in entries {
        by_file.entry(ep.node.path.clone()).or_default().push(ep);
    }
    let mut files: Vec<&String> = by_file.keys().collect();
    files.sort();
    for f in files {
        let _ = writeln!(out, "### `{}`\n", f);
        for ep in &by_file[f] {
            let _ = writeln!(
                out,
                "- `{}` ({}) — {}",
                ep.node.name, ep.node.kind, ep.reason
            );
        }
        out.push('\n');
    }
}

// =====================================================================
// 40-Risk/
// =====================================================================

fn write_risk(
    output_dir: &Path,
    db: &GraphDb,
    all_nodes: &[Node],
    opts: &DocsOptions,
) -> anyhow::Result<usize> {
    let dir = output_dir.join("40-Risk");
    std::fs::create_dir_all(&dir)?;

    let mut hs: Vec<&Node> = all_nodes
        .iter()
        .filter(|n| n.kind == "File" && n.churn > 0.0)
        .collect();
    hs.sort_by(|a, b| {
        let sa = a.churn * a.coupling + a.in_degree as f64 * 0.01;
        let sb = b.churn * b.coupling + b.in_degree as f64 * 0.01;
        sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut content = String::new();
    if opts.frontmatter {
        content.push_str("---\ncgx_kind: hotspots\ntags: [risk, cgx]\n---\n\n");
    }
    content.push_str("# Hotspots\n\n");
    content.push_str("Files ranked by churn × coupling. Edit these carefully.\n\n");
    content.push_str("| File | Churn | Coupling | In-deg |\n|---|---|---|---|\n");
    for n in hs.iter().take(30) {
        let _ = writeln!(
            content,
            "| `{}` | {:.2} | {:.2} | {} |",
            n.path, n.churn, n.coupling, n.in_degree
        );
    }
    std::fs::write(dir.join("Hotspots.md"), content)?;

    let complex: Vec<&Node> = {
        let mut v: Vec<&Node> = all_nodes
            .iter()
            .filter(|n| n.kind == "Function" && n.complexity > 0.0)
            .collect();
        v.sort_by(|a, b| {
            b.complexity
                .partial_cmp(&a.complexity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        v
    };
    let mut content = String::new();
    if opts.frontmatter {
        content.push_str("---\ncgx_kind: complexity\ntags: [risk, cgx]\n---\n\n");
    }
    content.push_str("# High-complexity functions\n\n");
    content.push_str("Top cyclomatic-complexity hot spots — good refactor candidates.\n\n");
    if complex.is_empty() {
        content.push_str("_(no complexity scores stored — cgx only computes complexity for TypeScript right now.)_\n");
    } else {
        content.push_str("| Function | File | Complexity |\n|---|---|---|\n");
        for n in complex.iter().take(40) {
            let _ = writeln!(
                content,
                "| `{}` | `{}` | {:.1} |",
                n.name, n.path, n.complexity
            );
        }
    }
    std::fs::write(dir.join("ComplexityHigh.md"), content)?;

    if opts.include_dead_code {
        let dead: Vec<&Node> = all_nodes.iter().filter(|n| n.is_dead_candidate).collect();
        let mut content = String::new();
        if opts.frontmatter {
            content.push_str("---\ncgx_kind: dead_code\ntags: [risk, cleanup, cgx]\n---\n\n");
        }
        content.push_str("# Dead code candidates\n\n");
        if dead.is_empty() {
            content.push_str("_(no dead-code candidates flagged)_\n");
        } else {
            content.push_str("| Symbol | Kind | File | Reason |\n|---|---|---|---|\n");
            for n in &dead {
                let _ = writeln!(
                    content,
                    "| `{}` | {} | `{}` | {} |",
                    n.name,
                    n.kind,
                    n.path,
                    n.dead_reason.clone().unwrap_or_default()
                );
            }
        }
        std::fs::write(dir.join("DeadCode.md"), content)?;
    }

    if opts.include_duplicates {
        let clones = db.get_clones(0.85, None).unwrap_or_default();
        let mut content = String::new();
        if opts.frontmatter {
            content.push_str("---\ncgx_kind: duplicates\ntags: [risk, cgx]\n---\n\n");
        }
        content.push_str("# Duplicate / near-duplicate code\n\n");
        if clones.is_empty() {
            content.push_str("_(no clone pairs above similarity threshold)_\n");
        } else {
            content.push_str("| A | B | Similarity | Kind |\n|---|---|---|---|\n");
            for c in clones.iter().take(100) {
                let _ = writeln!(
                    content,
                    "| `{}` | `{}` | {:.2} | {} |",
                    c.node_a, c.node_b, c.similarity, c.kind
                );
            }
        }
        std::fs::write(dir.join("Duplicates.md"), content)?;
    }

    let total = 2
        + if opts.include_dead_code { 1 } else { 0 }
        + if opts.include_duplicates { 1 } else { 0 };
    Ok(total)
}

// =====================================================================
// 50-Ownership/
// =====================================================================

fn write_ownership(
    output_dir: &Path,
    _db: &GraphDb,
    all_nodes: &[Node],
    opts: &DocsOptions,
) -> anyhow::Result<usize> {
    let dir = output_dir.join("50-Ownership");
    std::fs::create_dir_all(&dir)?;

    let authors: Vec<&Node> = all_nodes.iter().filter(|n| n.kind == "Author").collect();
    let mut content = String::new();
    if opts.frontmatter {
        content.push_str("---\ncgx_kind: owners\ntags: [ownership, cgx]\n---\n\n");
    }
    content.push_str("# File owners\n\n");
    if authors.is_empty() {
        content.push_str("_(no author data — git layer may have been disabled during analyze)_\n");
    } else {
        let _ = writeln!(content, "{} distinct contributor(s).\n", authors.len());
        for a in &authors {
            let _ = writeln!(content, "- {}", a.name);
        }
    }
    std::fs::write(dir.join("Owners.md"), content)?;

    let mut content = String::new();
    if opts.frontmatter {
        content.push_str("---\ncgx_kind: blame_graph\ntags: [ownership, cgx]\n---\n\n");
    }
    content.push_str("# Blame graph\n\n");
    content.push_str("Run `cgx query blame-graph` for the live per-contributor breakdown.\n");
    std::fs::write(dir.join("BlameGraph.md"), content)?;

    Ok(2)
}

// =====================================================================
// README.md (root index)
// =====================================================================

fn write_readme(
    output_dir: &Path,
    db: &GraphDb,
    file_paths: &[String],
    project: &ProjectInfo,
    summaries: &[(String, FileSummary, FileRole)],
    opts: &DocsOptions,
) -> anyhow::Result<usize> {
    let mut readme = String::new();
    if opts.frontmatter {
        readme.push_str("---\ncgx_kind: index\ntags: [cgx]\n---\n\n");
    }
    readme.push_str("# cgx docs\n\n");

    if let Some(name) = &project.name {
        let _ = writeln!(readme, "Auto-generated documentation for **`{}`**.\n", name);
    }
    if let Some(desc) = &project.description {
        let _ = writeln!(readme, "> {}\n", desc);
    } else if let Some(rd) = &project.readme_excerpt {
        let _ = writeln!(readme, "> {}\n", rd);
    }

    if !project.stack.is_empty() {
        let _ = writeln!(readme, "**Stack:** {}", project.stack.join(", "));
    }
    let _ = writeln!(
        readme,
        "**Size:** {} files · {} nodes · {} edges · {} declared deps · regenerated {}\n",
        file_paths.len(),
        db.node_count().unwrap_or(0),
        db.edge_count().unwrap_or(0),
        project.deps.len(),
        chrono::Utc::now().format("%Y-%m-%d %H:%M UTC"),
    );

    // Quick dep highlight: top runtime deps and any flagged-unused ones.
    let project_prefix = project.name.as_deref().unwrap_or("").to_string();
    let workspace_prefix = project_prefix.split('-').next().unwrap_or("").to_string();
    let is_intra_workspace = |name: &str| -> bool {
        !workspace_prefix.is_empty() && name.starts_with(&format!("{}-", workspace_prefix))
    };
    let runtime: Vec<&Dependency> = project
        .deps
        .iter()
        .filter(|d| matches!(d.kind, DepKind::Runtime))
        .collect();
    let curated: Vec<&Dependency> = runtime
        .iter()
        .copied()
        .filter(|d| !is_intra_workspace(&d.name) && !d.purpose.starts_with("Uncategorised"))
        .collect();
    let intra: Vec<&Dependency> = runtime
        .iter()
        .copied()
        .filter(|d| is_intra_workspace(&d.name))
        .collect();

    if !runtime.is_empty() {
        readme.push_str("## What this project pulls in\n\n");
        readme.push_str(
            "Highlights — full breakdown in [[00-Overview/Architecture|Architecture overview]].\n\n",
        );
        for d in curated.iter().take(12) {
            let _ = writeln!(readme, "- `{}` — {}", d.name, d.purpose);
        }
        if !intra.is_empty() {
            readme.push_str("\n**Workspace-internal:** ");
            let names: Vec<String> = intra.iter().map(|d| format!("`{}`", d.name)).collect();
            readme.push_str(&names.join(", "));
            readme.push('\n');
        }
        let unused: Vec<&Dependency> = runtime
            .iter()
            .copied()
            .filter(|d| !d.used && !is_intra_workspace(&d.name))
            .collect();
        if !unused.is_empty() {
            readme.push('\n');
            let names: Vec<String> = unused.iter().map(|d| format!("`{}`", d.name)).collect();
            let _ = writeln!(
                readme,
                "> ⚠ **Possibly unused:** {} — declared in a manifest but no `use`/`import` found. Verify before removing.\n",
                names.join(", ")
            );
        }
    }

    readme.push_str("## Start here\n\n");
    let _ = writeln!(
        readme,
        "- {} — what this project is, in 30 seconds.",
        format_link(
            "00-Overview/Architecture",
            "Architecture overview",
            opts.wiki_links_style
        )
    );
    let _ = writeln!(
        readme,
        "- {} — recommended reading path for new readers.",
        format_link(
            "00-Overview/HowToNavigate",
            "How to navigate",
            opts.wiki_links_style
        )
    );
    readme.push('\n');

    readme.push_str("## Everything else\n\n");
    for (target, alias) in [
        ("00-Overview/Glossary", "Glossary"),
        (
            "20-Architecture/Groups",
            "Source groups (best place to browse)",
        ),
        ("10-PublicAPI", "Public APIs (per group)"),
        (
            "20-Architecture/Communities",
            "Louvain communities (graph-clustered)",
        ),
        (
            "20-Architecture/CrossClusterDeps",
            "Cross-cluster dependencies",
        ),
        ("20-Architecture/EntryPoints", "Entry points"),
        ("40-Risk/Hotspots", "Hotspots"),
        ("40-Risk/ComplexityHigh", "High-complexity functions"),
        ("40-Risk/DeadCode", "Dead code"),
        ("40-Risk/Duplicates", "Duplicates"),
        ("50-Ownership/Owners", "File owners"),
    ] {
        let _ = writeln!(
            readme,
            "- {}",
            format_link(target, alias, opts.wiki_links_style)
        );
    }

    // Files by role — quick orientation.
    let role_counts = role_distribution(summaries);
    if !role_counts.is_empty() {
        readme.push_str("\n## Files by role\n\n");
        let mut ordered: Vec<_> = role_counts.iter().collect();
        ordered.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
        for (role, count) in ordered {
            let _ = writeln!(readme, "- **{}** — {}", role, count);
        }
    }

    readme.push_str("\n## Filling in the prose\n\n");
    readme.push_str(
        "Each module note under `30-Modules/` ends with a `<!-- cgx-prompt -->` block. \
         Paste it into Claude / Cursor and replace the block with the response.\n\n\
         Or run `cgx docs prompts` to list unfilled stubs and `cgx docs prompts --next` to print one packet at a time.\n",
    );

    std::fs::write(output_dir.join("README.md"), readme)?;
    Ok(1)
}

// =====================================================================
// Helpers
// =====================================================================

fn role_distribution(
    summaries: &[(String, FileSummary, FileRole)],
) -> HashMap<&'static str, usize> {
    let mut counts: HashMap<&'static str, usize> = HashMap::new();
    for (_, _, role) in summaries {
        *counts.entry(role.label()).or_insert(0) += 1;
    }
    counts
}

fn first_line(s: &str) -> String {
    s.lines()
        .next()
        .map(|l| l.trim().to_string())
        .unwrap_or_default()
}

fn escape_table_cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn dedup_keep_order<I, T>(it: I) -> Vec<T>
where
    I: IntoIterator<Item = T>,
    T: Eq + std::hash::Hash + Clone,
{
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for v in it {
        if seen.insert(v.clone()) {
            out.push(v);
        }
    }
    out
}

fn yaml_quote(raw: &str) -> String {
    let needs_quote = raw.is_empty()
        || raw.starts_with('-')
        || raw.contains(": ")
        || raw.contains(['[', ']', '{', '}', ',', '#', '"', '\n', '\r', '\t']);
    if !needs_quote {
        return raw.to_string();
    }
    let mut out = String::with_capacity(raw.len() + 2);
    out.push('"');
    for c in raw.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}
