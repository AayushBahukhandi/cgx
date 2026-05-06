use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

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
    Unknown,
}

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: PathBuf,
    pub relative_path: String,
    pub language: Language,
    pub content: String,
    pub size_bytes: u64,
}

pub fn walk_repo(repo_path: &Path) -> anyhow::Result<Vec<SourceFile>> {
    let mut files = Vec::new();
    let canonical = repo_path.canonicalize()?;

    let mut walker = WalkBuilder::new(&canonical);
    walker.standard_filters(true);
    walker.hidden(true);
    // Explicitly add overrides for common non-source directories
    let mut override_builder = ignore::overrides::OverrideBuilder::new(&canonical);
    for pattern in &["!node_modules/", "!target/", "!dist/", "!__pycache__/", "!.git/"] {
        let _ = override_builder.add(pattern);
    }
    let overrides = override_builder.build()?;
    walker.overrides(overrides);

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
