//! Detect "what is this project?" by walking every manifest in the repo and
//! cross-referencing declared dependencies with actual imports in source code.
//!
//! Output drives both `00-Overview/Architecture.md` and the root `README.md`.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct ProjectInfo {
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    /// Free-form labels: "Rust workspace", "Node package", "Python package", "Go module".
    pub stack: Vec<String>,
    /// First non-empty paragraph from README.md.
    pub readme_excerpt: Option<String>,
    /// One entry per `(manifest, dep)` pair, deduped by name.
    pub deps: Vec<Dependency>,
    /// Manifest files we successfully read, relative to repo root.
    pub manifests: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Dependency {
    pub name: String,
    pub version: Option<String>,
    pub manifest: String,
    pub kind: DepKind,
    /// Human-readable one-liner ("JSON serialisation"). Curated lookup with a fallback.
    pub purpose: String,
    /// Whether we found any imports of this dep in source files.
    pub used: bool,
    /// Number of files importing this dep (0 ⇒ unused candidate).
    pub use_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepKind {
    Runtime,
    Dev,
    Build,
    Peer,
}

impl DepKind {
    pub fn label(self) -> &'static str {
        match self {
            DepKind::Runtime => "runtime",
            DepKind::Dev => "dev",
            DepKind::Build => "build",
            DepKind::Peer => "peer",
        }
    }
}

pub fn detect(repo_path: &Path) -> ProjectInfo {
    let mut info = ProjectInfo::default();

    // ---- Walk every relevant manifest ----
    let manifests = discover_manifests(repo_path);
    let mut raw_deps: Vec<Dependency> = Vec::new();

    for manifest in &manifests {
        let abs = repo_path.join(manifest);
        let rel = manifest.clone();
        if manifest.ends_with("Cargo.toml") {
            if let Some((header, deps)) = read_cargo(&abs, &rel) {
                merge_header(&mut info, header, "Rust workspace");
                raw_deps.extend(deps);
            }
        } else if manifest.ends_with("package.json") {
            if let Some((header, deps)) = read_package_json(&abs, &rel) {
                merge_header(&mut info, header, "Node project");
                raw_deps.extend(deps);
            }
        } else if manifest.ends_with("pyproject.toml") {
            if let Some((header, deps)) = read_pyproject(&abs, &rel) {
                merge_header(&mut info, header, "Python package");
                raw_deps.extend(deps);
            }
        } else if manifest.ends_with("requirements.txt") {
            let deps = read_requirements_txt(&abs, &rel);
            if !deps.is_empty() {
                if !info.stack.contains(&"Python package".to_string()) {
                    info.stack.push("Python package".to_string());
                }
                raw_deps.extend(deps);
            }
        } else if manifest.ends_with("go.mod")
            && read_go_mod(&abs).is_some()
            && !info.stack.contains(&"Go module".to_string())
        {
            info.stack.push("Go module".to_string());
        }
        info.manifests.push(rel);
    }

    // ---- Scan source for imports to flag unused deps ----
    let index = scan_imports(repo_path);

    // ---- Dedupe by name (workspace + sub-crate often declare the same dep) ----
    let mut by_name: BTreeMap<String, Dependency> = BTreeMap::new();
    for mut d in raw_deps {
        d.use_count = lookup_use_count(&index, &d.name);
        d.used = d.use_count > 0;
        d.purpose = purpose_for(&d.name);
        by_name
            .entry(d.name.clone())
            .and_modify(|existing| {
                // Prefer the entry with the more specific manifest path (longer one is sub-crate).
                if d.manifest.len() > existing.manifest.len() {
                    existing.manifest = d.manifest.clone();
                }
                if existing.version.is_none() && d.version.is_some() {
                    existing.version = d.version.clone();
                }
                if d.used && !existing.used {
                    existing.used = true;
                    existing.use_count = d.use_count;
                }
            })
            .or_insert(d);
    }
    info.deps = by_name.into_values().collect();
    info.deps.sort_by(|a, b| a.name.cmp(&b.name));

    info.readme_excerpt = read_readme_excerpt(repo_path);

    // Stable dedup of stack labels.
    let mut seen = HashSet::new();
    info.stack.retain(|s| seen.insert(s.clone()));

    info
}

fn discover_manifests(repo_path: &Path) -> Vec<String> {
    let candidates = [
        "Cargo.toml",
        "package.json",
        "pyproject.toml",
        "requirements.txt",
        "go.mod",
        "Pipfile",
    ];
    let mut out: Vec<String> = Vec::new();
    for c in &candidates {
        if repo_path.join(c).exists() {
            out.push((*c).to_string());
        }
    }
    // Walk one level deep into common workspace folders.
    for prefix in ["crates", "packages", "apps", "services"] {
        let dir = repo_path.join(prefix);
        if !dir.is_dir() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            for c in &candidates {
                let p = entry.path().join(c);
                if p.exists() {
                    if let Ok(rel) = p.strip_prefix(repo_path) {
                        out.push(rel.to_string_lossy().to_string());
                    }
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

#[derive(Default)]
struct ManifestHeader {
    name: Option<String>,
    version: Option<String>,
    description: Option<String>,
}

fn merge_header(info: &mut ProjectInfo, header: ManifestHeader, stack_label: &str) {
    if info.name.is_none() {
        info.name = header.name;
    }
    if info.version.is_none() {
        info.version = header.version;
    }
    if info.description.is_none() {
        info.description = header.description;
    }
    if !info.stack.iter().any(|s| s == stack_label) {
        info.stack.push(stack_label.to_string());
    }
}

// ---------------------------------------------------------------------
// Per-format readers
// ---------------------------------------------------------------------

fn read_cargo(abs: &Path, rel: &str) -> Option<(ManifestHeader, Vec<Dependency>)> {
    let content = std::fs::read_to_string(abs).ok()?;
    let parsed: toml::Value = toml::from_str(&content).ok()?;

    let pkg = parsed.get("package");
    let workspace_pkg = parsed.get("workspace").and_then(|w| w.get("package"));
    let source = pkg.or(workspace_pkg);
    let header = ManifestHeader {
        name: source
            .and_then(|t| t.get("name"))
            .and_then(|v| v.as_str())
            .map(String::from),
        version: source
            .and_then(|t| t.get("version"))
            .and_then(|v| v.as_str())
            .map(String::from),
        description: source
            .and_then(|t| t.get("description"))
            .and_then(|v| v.as_str())
            .map(String::from),
    };

    let mut deps: Vec<Dependency> = Vec::new();
    for (table_path, kind) in [
        (vec!["dependencies"], DepKind::Runtime),
        (vec!["dev-dependencies"], DepKind::Dev),
        (vec!["build-dependencies"], DepKind::Build),
        (vec!["workspace", "dependencies"], DepKind::Runtime),
    ] {
        let mut node: &toml::Value = &parsed;
        let mut ok = true;
        for seg in &table_path {
            match node.get(*seg) {
                Some(n) => node = n,
                None => {
                    ok = false;
                    break;
                }
            }
        }
        if !ok {
            continue;
        }
        if let Some(map) = node.as_table() {
            for (name, value) in map {
                let version = match value {
                    toml::Value::String(s) => Some(s.clone()),
                    toml::Value::Table(t) => {
                        t.get("version").and_then(|v| v.as_str()).map(String::from)
                    }
                    _ => None,
                };
                deps.push(Dependency {
                    name: name.clone(),
                    version,
                    manifest: rel.to_string(),
                    kind,
                    purpose: String::new(),
                    used: false,
                    use_count: 0,
                });
            }
        }
    }

    Some((header, deps))
}

fn read_package_json(abs: &Path, rel: &str) -> Option<(ManifestHeader, Vec<Dependency>)> {
    let content = std::fs::read_to_string(abs).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&content).ok()?;
    let header = ManifestHeader {
        name: parsed
            .get("name")
            .and_then(|v| v.as_str())
            .map(String::from),
        version: parsed
            .get("version")
            .and_then(|v| v.as_str())
            .map(String::from),
        description: parsed
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from),
    };

    let mut deps: Vec<Dependency> = Vec::new();
    for (key, kind) in [
        ("dependencies", DepKind::Runtime),
        ("devDependencies", DepKind::Dev),
        ("peerDependencies", DepKind::Peer),
    ] {
        if let Some(map) = parsed.get(key).and_then(|v| v.as_object()) {
            for (name, ver) in map {
                deps.push(Dependency {
                    name: name.clone(),
                    version: ver.as_str().map(String::from),
                    manifest: rel.to_string(),
                    kind,
                    purpose: String::new(),
                    used: false,
                    use_count: 0,
                });
            }
        }
    }
    Some((header, deps))
}

fn read_pyproject(abs: &Path, rel: &str) -> Option<(ManifestHeader, Vec<Dependency>)> {
    let content = std::fs::read_to_string(abs).ok()?;
    let parsed: toml::Value = toml::from_str(&content).ok()?;
    let project = parsed.get("project")?;
    let header = ManifestHeader {
        name: project
            .get("name")
            .and_then(|v| v.as_str())
            .map(String::from),
        version: project
            .get("version")
            .and_then(|v| v.as_str())
            .map(String::from),
        description: project
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from),
    };

    let mut deps: Vec<Dependency> = Vec::new();
    if let Some(list) = project.get("dependencies").and_then(|v| v.as_array()) {
        for d in list {
            if let Some(s) = d.as_str() {
                let name = s
                    .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
                    .next()
                    .unwrap_or("");
                if !name.is_empty() {
                    deps.push(Dependency {
                        name: name.to_string(),
                        version: None,
                        manifest: rel.to_string(),
                        kind: DepKind::Runtime,
                        purpose: String::new(),
                        used: false,
                        use_count: 0,
                    });
                }
            }
        }
    }
    Some((header, deps))
}

fn read_requirements_txt(abs: &Path, rel: &str) -> Vec<Dependency> {
    let Ok(content) = std::fs::read_to_string(abs) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let name = line
            .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
            .next()
            .unwrap_or("");
        if name.is_empty() {
            continue;
        }
        out.push(Dependency {
            name: name.to_string(),
            version: None,
            manifest: rel.to_string(),
            kind: DepKind::Runtime,
            purpose: String::new(),
            used: false,
            use_count: 0,
        });
    }
    out
}

fn read_go_mod(abs: &Path) -> Option<()> {
    if abs.exists() {
        Some(())
    } else {
        None
    }
}

// ---------------------------------------------------------------------
// README excerpt
// ---------------------------------------------------------------------

fn read_readme_excerpt(repo_path: &Path) -> Option<String> {
    for name in ["README.md", "Readme.md", "readme.md", "README.markdown"] {
        let p = repo_path.join(name);
        if !p.exists() {
            continue;
        }
        let content = std::fs::read_to_string(&p).ok()?;
        let body = if content.starts_with("---") {
            content.splitn(3, "---").nth(2).unwrap_or(&content)
        } else {
            &content
        };
        let mut paragraph: Vec<&str> = Vec::new();
        for line in body.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                if !paragraph.is_empty() {
                    let joined = paragraph.join(" ").trim().to_string();
                    if !joined.is_empty() && joined.len() > 20 {
                        return Some(truncate_clean(&joined, 600));
                    }
                    paragraph.clear();
                }
                continue;
            }
            if trimmed.starts_with('#')
                || trimmed.starts_with('!')
                || trimmed.starts_with('<')
                || trimmed.starts_with("> ")
                || trimmed.starts_with('|')
                || trimmed.starts_with("```")
            {
                paragraph.clear();
                continue;
            }
            paragraph.push(trimmed);
        }
        if !paragraph.is_empty() {
            let joined = paragraph.join(" ").trim().to_string();
            if !joined.is_empty() && joined.len() > 20 {
                return Some(truncate_clean(&joined, 600));
            }
        }
    }
    None
}

fn truncate_clean(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let slice = &s[..max];
    if let Some(idx) = slice.rfind(['.', '!', '?']) {
        return s[..=idx].to_string();
    }
    format!("{}…", slice)
}

// ---------------------------------------------------------------------
// Import scanning
// ---------------------------------------------------------------------

/// Lightweight per-language index of source files for usage detection.
struct SourceIndex {
    /// `(ext, content)` for each readable source file.
    files: Vec<(String, String)>,
}

fn scan_imports(repo_path: &Path) -> SourceIndex {
    let mut paths: Vec<PathBuf> = Vec::new();
    walk_for_source(repo_path, &mut paths, 0);
    let mut files: Vec<(String, String)> = Vec::with_capacity(paths.len());
    for p in paths {
        let Some(ext) = p.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        let Ok(content) = std::fs::read_to_string(&p) else {
            continue;
        };
        files.push((ext.to_string(), content));
    }
    SourceIndex { files }
}

fn walk_for_source(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > 10 {
        return;
    }
    let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if matches!(
        name,
        "node_modules" | "target" | ".git" | "dist" | "build" | "web-ui-dist" | "__pycache__"
    ) {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_for_source(&path, out, depth + 1);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if matches!(
                ext,
                "rs" | "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "py" | "go"
            ) {
                out.push(path);
            }
        }
    }
}

/// Count files in the index that "use" `dep_name`.
///
/// For Rust, we look for the identifier (with `-` → `_`) appearing as a
/// word boundary anywhere in the source — this catches both `use foo::...`
/// and bare path expressions like `foo::bar()` that `use`-only scanning
/// misses (e.g. `tree_sitter_rust::language()`, `serde_json::json!(...)`).
///
/// For JS/TS we still look for `import` / `require` strings since identifier
/// names in JS don't reliably match the package name.
fn lookup_use_count(index: &SourceIndex, dep_name: &str) -> usize {
    let rust_ident = dep_name.replace('-', "_");
    let mut count = 0usize;
    for (ext, content) in &index.files {
        let matched = match ext.as_str() {
            "rs" => rust_file_uses(content, &rust_ident),
            "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" => js_file_imports(content, dep_name),
            "py" => python_file_imports(content, dep_name),
            "go" => go_file_imports(content, dep_name),
            _ => false,
        };
        if matched {
            count += 1;
        }
    }
    count
}

/// True if `content` contains `ident` as an identifier (word-bounded).
/// Skips matches inside line/block comments so a stray "// uses tokio" doesn't
/// count as real usage.
fn rust_file_uses(content: &str, ident: &str) -> bool {
    if ident.is_empty() {
        return false;
    }
    // Strip line and block comments before scanning.
    let stripped = strip_rust_comments(content);
    contains_identifier(&stripped, ident)
}

fn js_file_imports(content: &str, pkg: &str) -> bool {
    for line in content.lines() {
        let t = line.trim_start();
        for marker in ["from ", "import ", "require("] {
            if let Some(idx) = t.find(marker) {
                let after = &t[idx + marker.len()..];
                let after = after.trim_start();
                if let Some(stripped) = after.strip_prefix('\'').or_else(|| after.strip_prefix('"'))
                {
                    if let Some(end) = stripped.find(['\'', '"']) {
                        let name = &stripped[..end];
                        if name == pkg || name.starts_with(&format!("{}/", pkg)) {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

fn python_file_imports(content: &str, pkg: &str) -> bool {
    for line in content.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("from ") {
            let head = rest.split([' ', '.']).next().unwrap_or("");
            if head == pkg {
                return true;
            }
        } else if let Some(rest) = t.strip_prefix("import ") {
            for part in rest.split(',') {
                let head = part.trim().split([' ', '.', ';']).next().unwrap_or("");
                if head == pkg {
                    return true;
                }
            }
        }
    }
    false
}

fn go_file_imports(content: &str, pkg: &str) -> bool {
    // Go deps are written as full module paths; do a contains check.
    for line in content.lines() {
        let t = line.trim();
        if let Some(idx) = t.find('"') {
            if let Some(end) = t[idx + 1..].find('"') {
                let path = &t[idx + 1..idx + 1 + end];
                if path == pkg || path.starts_with(&format!("{}/", pkg)) {
                    return true;
                }
            }
        }
    }
    false
}

/// Word-boundary identifier check. `ident` is treated as a Rust-style identifier.
fn contains_identifier(haystack: &str, ident: &str) -> bool {
    let mut start = 0;
    while let Some(pos) = haystack[start..].find(ident) {
        let abs = start + pos;
        let before = if abs == 0 {
            None
        } else {
            haystack[..abs].chars().last()
        };
        let after = haystack[abs + ident.len()..].chars().next();
        let ok_before = before.is_none_or(|c| !is_ident_char(c));
        let ok_after = after.is_none_or(|c| !is_ident_char(c));
        if ok_before && ok_after {
            return true;
        }
        start = abs + ident.len();
    }
    false
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Strip `//`-line and `/* */`-block comments from Rust source. Naive but
/// good enough for usage detection — string literals containing `//` are
/// rare in practice.
fn strip_rust_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            // line comment
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            // block comment (no nesting for simplicity)
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = i.saturating_add(2).min(bytes.len());
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

// ---------------------------------------------------------------------
// Curated purpose lookup
// ---------------------------------------------------------------------

const PURPOSES: &[(&str, &str)] = &[
    // --- Rust ---
    ("anyhow", "Error handling with context chains"),
    ("thiserror", "Custom error types"),
    ("serde", "Serialise/deserialise framework"),
    ("serde_json", "JSON serialisation"),
    ("toml", "TOML parsing"),
    ("tokio", "Async runtime"),
    ("async-trait", "Async methods in traits"),
    ("futures", "Async combinators"),
    ("tracing", "Structured logging"),
    ("tracing-subscriber", "Logging output formatting"),
    ("log", "Logging facade"),
    ("env_logger", "Environment-driven logger"),
    ("chrono", "Date and time"),
    ("clap", "CLI argument parsing"),
    ("dirs", "Standard user directories"),
    ("sha2", "SHA-256/512 hashing"),
    ("blake3", "Fast cryptographic hashing"),
    ("uuid", "UUID generation"),
    ("regex", "Regular expressions"),
    ("rayon", "Data parallelism"),
    ("crossbeam", "Concurrent data structures"),
    ("notify", "Filesystem watching"),
    ("walkdir", "Recursive directory walking"),
    ("ignore", "Gitignore-aware file walking"),
    ("duckdb", "Embedded analytics database"),
    ("rusqlite", "SQLite bindings"),
    ("reqwest", "HTTP client"),
    ("axum", "HTTP server"),
    ("actix-web", "HTTP server"),
    ("hyper", "Low-level HTTP"),
    ("tower", "Service middleware"),
    ("ratatui", "Terminal UI framework"),
    ("crossterm", "Terminal manipulation"),
    ("tree-sitter", "Incremental parsing framework"),
    ("tree-sitter-rust", "Tree-sitter Rust grammar"),
    ("tree-sitter-typescript", "Tree-sitter TypeScript grammar"),
    ("tree-sitter-javascript", "Tree-sitter JavaScript grammar"),
    ("tree-sitter-python", "Tree-sitter Python grammar"),
    ("tree-sitter-go", "Tree-sitter Go grammar"),
    ("tree-sitter-java", "Tree-sitter Java grammar"),
    ("tree-sitter-php", "Tree-sitter PHP grammar"),
    ("tree-sitter-c-sharp", "Tree-sitter C# grammar"),
    ("git2", "Git bindings"),
    ("gix", "Pure-Rust git"),
    ("indicatif", "Progress bars"),
    ("dialoguer", "Interactive prompts"),
    ("console", "Terminal styling"),
    ("colored", "Terminal colours"),
    ("once_cell", "Lazy statics"),
    ("lazy_static", "Lazy statics"),
    ("itertools", "Iterator helpers"),
    ("rand", "Random number generation"),
    ("fastrand", "Fast random number generation"),
    ("base64", "Base64 encoding"),
    ("flate2", "DEFLATE/gzip compression"),
    ("mime_guess", "MIME-type guessing from path"),
    ("open", "Open a path in the user's default app"),
    ("rust-embed", "Embed files into the binary at build time"),
    ("tokio-stream", "Async stream combinators (Tokio)"),
    ("tokio-util", "Tokio helpers (codecs, frames)"),
    ("tower-http", "HTTP middleware (tower)"),
    ("async-stream", "Async stream macros"),
    ("fdg-sim", "Force-directed graph simulation"),
    ("graphology", "Graph data structures (JS)"),
    ("graphology-communities-louvain", "Louvain clustering (JS)"),
    ("graphology-layout-forceatlas2", "ForceAtlas2 layout (JS)"),
    ("sigma", "Graph rendering (JS)"),
    ("prism-react-renderer", "Syntax highlighting"),
    // --- Node / JS / TS ---
    ("react", "UI rendering library"),
    ("react-dom", "React DOM renderer"),
    ("react-router", "Client-side routing"),
    ("react-router-dom", "React routing for browsers"),
    ("next", "Next.js framework"),
    ("vue", "Vue.js framework"),
    ("svelte", "Svelte framework"),
    ("vite", "Frontend dev server / bundler"),
    ("@vitejs/plugin-react", "Vite React plugin"),
    ("typescript", "TypeScript compiler"),
    ("eslint", "Linter"),
    ("prettier", "Code formatter"),
    ("tailwindcss", "CSS utility framework"),
    ("postcss", "CSS post-processor"),
    ("autoprefixer", "CSS vendor prefixing"),
    ("d3", "Data-driven SVG"),
    ("d3-force", "Force-directed layout"),
    ("d3-zoom", "SVG pan/zoom"),
    ("fuse.js", "Client-side fuzzy search"),
    ("zustand", "State management"),
    ("@tanstack/react-query", "Async data fetching cache"),
    ("axios", "HTTP client"),
    ("lodash", "Utility helpers"),
    ("zod", "Schema validation"),
    ("dayjs", "Date manipulation"),
    ("framer-motion", "Animation library"),
    ("clsx", "Conditional className helper"),
    ("vitest", "Test runner"),
    ("jest", "Test runner"),
    ("@testing-library/react", "React testing utilities"),
    ("@types/node", "Node.js TypeScript types"),
    ("@types/react", "React TypeScript types"),
    ("@types/react-dom", "React DOM TypeScript types"),
    // --- Python ---
    ("requests", "HTTP client"),
    ("flask", "Web framework"),
    ("django", "Web framework"),
    ("fastapi", "ASGI web framework"),
    ("pydantic", "Data validation"),
    ("sqlalchemy", "ORM"),
    ("numpy", "Numerical arrays"),
    ("pandas", "Data analysis"),
    ("pytest", "Test runner"),
    // --- Go ---
    ("github.com/gin-gonic/gin", "Web framework"),
    ("github.com/spf13/cobra", "CLI framework"),
    ("github.com/stretchr/testify", "Testing assertions"),
];

fn purpose_for(name: &str) -> String {
    for (n, p) in PURPOSES {
        if *n == name {
            return (*p).to_string();
        }
    }
    // Heuristic fallback for unknown deps — extract a few tokens from the name.
    if let Some(grammar) = name.strip_prefix("tree-sitter-") {
        return format!("Tree-sitter grammar for {}", grammar);
    }
    if let Some(typename) = name.strip_prefix("@types/") {
        return format!("TypeScript types for `{}`", typename);
    }
    if name.starts_with("eslint-") {
        return "ESLint plugin".to_string();
    }
    if name.contains("logger") || name.contains("logging") {
        return "Logging".to_string();
    }
    "Uncategorised — see crate/package docs".to_string()
}
