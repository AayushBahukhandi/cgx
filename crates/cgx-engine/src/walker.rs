use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

/// Source language detected from file extension.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Language {
    TypeScript,
    JavaScript,
    Python,
    Rust,
    Go,
    Java,
    CSharp,
    Php,
    /// Extension not recognised — file is skipped by the parser.
    Unknown,
}

/// A source file that has been read from disk and is ready for parsing.
#[derive(Debug, Clone)]
pub struct SourceFile {
    /// Absolute path on disk.
    pub path: PathBuf,
    /// Path relative to the repo root, used as the stable identifier in the graph.
    pub relative_path: String,
    pub language: Language,
    pub content: String,
    pub size_bytes: u64,
}

/// Directory names that are never source code — skip them entirely.
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "dist",
    "__pycache__",
    ".git",
    ".next",
    "out",
    "coverage",
    "vendor",
    "venv",
    ".venv",
    ".tox",
    "build",
    "generated",
];

/// A directory name matches this if it ends with one of these suffixes (after a `-` or `_`).
const SKIP_DIR_SUFFIXES: &[&str] = &["-dist", "_dist", "-build", "_build", "-out", "_out"];

fn should_skip_dir(name: &str) -> bool {
    if SKIP_DIRS.contains(&name) {
        return true;
    }
    SKIP_DIR_SUFFIXES.iter().any(|suf| name.ends_with(suf))
}

/// Walk a repository and return every parseable source file.
///
/// Respects `.gitignore` and `.cgxignore` rules, skips build-artifact
/// directories (`target/`, `node_modules/`, `dist/`, …), binary files,
/// minified bundles, and files larger than 2 MB.
pub fn walk_repo(repo_path: &Path) -> anyhow::Result<Vec<SourceFile>> {
    let mut files = Vec::new();
    let canonical = repo_path.canonicalize()?;

    let mut walker = WalkBuilder::new(&canonical);
    walker.standard_filters(true);
    walker.hidden(true);
    // Respect .cgxignore files anywhere in the tree (same semantics as .gitignore)
    walker.add_custom_ignore_filename(".cgxignore");
    // Programmatic directory filter: prune entire build-artifact trees early
    walker.filter_entry(|e| {
        if e.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
            let name = e.file_name().to_string_lossy();
            // Also skip minified single-file bundles inside any directory
            !should_skip_dir(&name)
        } else {
            // Skip minified / bundled JS files by name convention
            let name = e.file_name().to_string_lossy();
            !name.ends_with(".min.js")
                && !name.ends_with(".min.ts")
                && !name.ends_with(".bundle.js")
                && !name.ends_with(".chunk.js")
        }
    });

    for entry in walker.build() {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
            continue;
        }

        let path = entry.path().to_path_buf();

        let metadata = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let size_bytes = metadata.len();

        if size_bytes > 2 * 1024 * 1024 {
            continue;
        }

        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        let language = detect_language(ext);

        if matches!(language, Language::Unknown) {
            continue;
        }

        if is_binary(&path)? {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let relative_path = match path.strip_prefix(&canonical) {
            Ok(r) => r.to_string_lossy().to_string(),
            Err(_) => path.to_string_lossy().to_string(),
        };

        // Belt-and-suspenders: reject files inside any excluded directory component
        if relative_path.split('/').any(should_skip_dir) {
            continue;
        }

        files.push(SourceFile {
            path,
            relative_path,
            language,
            content,
            size_bytes,
        });
    }

    Ok(files)
}

fn detect_language(ext: &str) -> Language {
    match ext {
        "ts" | "tsx" => Language::TypeScript,
        "js" | "jsx" | "mjs" | "cjs" => Language::JavaScript,
        "py" => Language::Python,
        "rs" => Language::Rust,
        "go" => Language::Go,
        "java" => Language::Java,
        "cs" => Language::CSharp,
        "php" => Language::Php,
        _ => Language::Unknown,
    }
}

fn is_binary(path: &Path) -> anyhow::Result<bool> {
    let mut file = std::fs::File::open(path)?;
    let mut buf = vec![0u8; 8192];
    let n = std::io::Read::read(&mut file, &mut buf).unwrap_or(0);
    Ok(buf[..n].contains(&0))
}
