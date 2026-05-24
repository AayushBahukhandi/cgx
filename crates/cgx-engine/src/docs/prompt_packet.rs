//! Builds the `<!-- cgx-prompt -->` block embedded in every module note.
//!
//! The packet inlines all the context an AI agent needs to produce prose for the
//! module without re-exploring the repo — listing exported symbols, in/out calls,
//! tests, owners, and any docstrings the parser already extracted.

use std::fmt::Write as _;

use crate::graph::{FileSummary, GraphDb};

/// Render the prompt block for a single file.
pub fn build(db: &GraphDb, summary: &FileSummary) -> String {
    let mut out = String::new();
    out.push_str("<!-- cgx-prompt:begin -->\n");
    let _ = writeln!(
        out,
        "You are documenting `{}` ({}).",
        summary.path,
        if summary.language.is_empty() {
            "unknown language"
        } else {
            &summary.language
        }
    );
    out.push_str("Write a 2–3 paragraph \"What this module does and why it exists\" section.\n\n");
    out.push_str("CONTEXT (do not re-read source — everything you need is below):\n");

    // Exported symbols (the public surface).
    let exported: Vec<&str> = summary
        .symbols
        .iter()
        .filter(|n| n.exported)
        .map(|n| n.name.as_str())
        .collect();
    if !exported.is_empty() {
        let _ = writeln!(out, "- Exported: {}", exported.join(", "));
    }

    // Non-exported / internal symbols (compact, name-only).
    let internal: Vec<&str> = summary
        .symbols
        .iter()
        .filter(|n| !n.exported && (n.kind == "Function" || n.kind == "Class"))
        .map(|n| n.name.as_str())
        .collect();
    if !internal.is_empty() {
        let preview: Vec<&str> = internal.iter().take(8).copied().collect();
        let suffix = if internal.len() > 8 {
            format!(" (+{} more)", internal.len() - 8)
        } else {
            String::new()
        };
        let _ = writeln!(out, "- Internal symbols: {}{}", preview.join(", "), suffix);
    }

    // Cross-file deps.
    if !summary.callees.is_empty() {
        let names: Vec<String> = summary
            .callees
            .iter()
            .take(8)
            .map(|n| format!("{} ({})", n.name, short_path(&n.path)))
            .collect();
        let _ = writeln!(out, "- Calls into: {}", names.join(", "));
    }
    if !summary.callers.is_empty() {
        let names: Vec<String> = summary
            .callers
            .iter()
            .take(8)
            .map(|n| format!("{} ({})", n.name, short_path(&n.path)))
            .collect();
        let _ = writeln!(out, "- Called by: {}", names.join(", "));
    }

    if !summary.tests.is_empty() {
        let paths: std::collections::BTreeSet<&str> =
            summary.tests.iter().map(|t| t.path.as_str()).collect();
        let _ = writeln!(
            out,
            "- Tests: {} ({} test fn{})",
            paths.iter().copied().collect::<Vec<_>>().join(", "),
            summary.tests.len(),
            if summary.tests.len() == 1 { "" } else { "s" }
        );
    }

    if !summary.owners.is_empty() {
        let mut seen = std::collections::HashSet::new();
        let names: Vec<String> = summary
            .owners
            .iter()
            .filter_map(|(n, _)| {
                if seen.insert(n.clone()) {
                    Some(n.clone())
                } else {
                    None
                }
            })
            .take(3)
            .collect();
        if !names.is_empty() {
            let _ = writeln!(out, "- Owners (top): {}", names.join(", "));
        }
    }

    let _ = writeln!(
        out,
        "- Metrics: complexity {:.1}, churn {:.2}, coupling {:.2}",
        summary.complexity, summary.churn, summary.coupling
    );

    // Existing docstrings — these are the most signal-dense input the AI gets.
    let mut docs_emitted = 0usize;
    out.push_str("- Existing docstrings:\n");
    for sym in &summary.symbols {
        if docs_emitted >= 10 {
            let _ = writeln!(out, "  - … (more symbols truncated)");
            break;
        }
        if let Ok(Some(doc)) = db.get_doc_comment(&sym.id) {
            if !doc.trim().is_empty() {
                let one_line: String = doc.lines().next().unwrap_or("").trim().to_string();
                let _ = writeln!(
                    out,
                    "  - {}: {}",
                    sym.name,
                    if one_line.is_empty() {
                        "(empty docstring)"
                    } else {
                        one_line.as_str()
                    }
                );
                docs_emitted += 1;
            }
        }
    }
    if docs_emitted == 0 {
        let _ = writeln!(
            out,
            "  (none — every symbol is undocumented at source level)"
        );
    }

    out.push_str(
        "\nOUTPUT: Replace this entire <!-- cgx-prompt --> block with your prose. \
         Keep any wiki-links elsewhere in the note intact.\n",
    );
    out.push_str("<!-- cgx-prompt:end -->\n");
    out
}

fn short_path(p: &str) -> &str {
    p.rsplit_once('/').map(|(_, base)| base).unwrap_or(p)
}
