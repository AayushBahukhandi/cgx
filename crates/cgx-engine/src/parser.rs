use std::collections::HashMap;

use crate::walker::{Language, SourceFile};

/// The semantic category of a graph node.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum NodeKind {
    /// A source file on disk.
    File,
    /// A named function or method.
    Function,
    /// A class, struct, or interface definition.
    Class,
    /// A module-level variable or constant.
    Variable,
    /// A type alias, interface, or enum definition.
    Type,
    /// A package or module (e.g. Go package, Python module).
    Module,
    /// A git commit author, used in ownership edges.
    Author,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CommentKind {
    /// Regular JS/JSDoc comment above a function or at the top level
    Standard,
    /// `{/* ... */}` expression comment inside JSX return body
    JsxExpression,
    /// JSX expression comment whose inner text starts with `<` — commented-out JSX code
    JsxCommentedCode,
}

impl CommentKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            CommentKind::Standard => "code",
            CommentKind::JsxExpression => "jsx",
            CommentKind::JsxCommentedCode => "jsx_commented_code",
        }
    }
}

/// A structured annotation extracted from a source comment (e.g. `@todo`, `@deprecated`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommentTag {
    /// Tag name, e.g. `"todo"`, `"fixme"`, `"hack"`.
    pub tag_type: String,
    /// Full comment text following the tag marker.
    pub text: String,
    pub line: u32,
    pub comment_kind: CommentKind,
}

impl NodeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeKind::File => "File",
            NodeKind::Function => "Function",
            NodeKind::Class => "Class",
            NodeKind::Variable => "Variable",
            NodeKind::Type => "Type",
            NodeKind::Module => "Module",
            NodeKind::Author => "Author",
        }
    }
}

/// The semantic relationship represented by a graph edge.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum EdgeKind {
    /// Function/method invocation.
    Calls,
    /// File imports another file or module.
    Imports,
    /// Class inherits from or implements another class/interface.
    Inherits,
    /// File exposes a symbol (file → function/class).
    Exports,
    /// Two files changed together in git history.
    CoChanges,
    /// Author owns a file (from git blame).
    Owns,
    /// File depends on an external package (from manifest parsing).
    DependsOn,
    /// Test file exercises a production symbol.
    Tests,
}

impl EdgeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            EdgeKind::Calls => "CALLS",
            EdgeKind::Imports => "IMPORTS",
            EdgeKind::Inherits => "INHERITS",
            EdgeKind::Exports => "EXPORTS",
            EdgeKind::CoChanges => "CO_CHANGES",
            EdgeKind::Owns => "OWNS",
            EdgeKind::DependsOn => "DEPENDS_ON",
            EdgeKind::Tests => "TESTS",
        }
    }
}

/// A node as produced by a language parser, before being written to the graph DB.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NodeDef {
    /// Stable, unique identifier — format: `<prefix>:<path>:<name>`, e.g. `fn:src/lib.rs:parse`.
    pub id: String,
    pub kind: NodeKind,
    pub name: String,
    /// Repo-relative file path.
    pub path: String,
    pub line_start: u32,
    pub line_end: u32,
    /// Parser-specific extras (e.g. `{"exported": true, "complexity": 4.0}`).
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl Default for NodeDef {
    fn default() -> Self {
        Self {
            id: String::new(),
            kind: NodeKind::File,
            name: String::new(),
            path: String::new(),
            line_start: 0,
            line_end: 0,
            metadata: serde_json::Value::Null,
        }
    }
}

/// A directed edge as produced by a language parser or the resolver.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EdgeDef {
    pub src: String,
    pub dst: String,
    pub kind: EdgeKind,
    /// Relative strength of the relationship (default 1.0).
    #[serde(default = "default_edge_weight")]
    pub weight: f64,
    /// Parser certainty that this edge is real, 0.0–1.0 (default 1.0, fuzzy matches use 0.8).
    #[serde(default = "default_edge_weight")]
    pub confidence: f64,
}

impl Default for EdgeDef {
    fn default() -> Self {
        Self {
            src: String::new(),
            dst: String::new(),
            kind: EdgeKind::Calls,
            weight: 1.0,
            confidence: 1.0,
        }
    }
}

fn default_edge_weight() -> f64 {
    1.0
}

/// The output of parsing a single source file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ParseResult {
    pub nodes: Vec<NodeDef>,
    pub edges: Vec<EdgeDef>,
    #[serde(default)]
    pub comment_tags: Vec<CommentTag>,
}

impl ParseResult {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            comment_tags: Vec::new(),
        }
    }
}

impl Default for ParseResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait implemented by every language-specific Tree-sitter parser.
pub trait LanguageParser: Send + Sync {
    /// File extensions this parser handles, e.g. `&["ts", "tsx"]`.
    fn extensions(&self) -> &[&str];
    /// Parse a source file and return all nodes, edges, and comment tags found.
    fn extract(&self, file: &SourceFile) -> anyhow::Result<ParseResult>;
}

/// Registry that maps [`Language`] variants to their [`LanguageParser`] implementations.
///
/// Constructed with all built-in parsers pre-registered.  Use [`ParserRegistry::parse`]
/// for a single file or [`ParserRegistry::parse_all`] for parallel batch processing.
pub struct ParserRegistry {
    parsers: HashMap<Language, Box<dyn LanguageParser>>,
}

impl ParserRegistry {
    /// Create a registry with all built-in language parsers registered.
    pub fn new() -> Self {
        let mut parsers: HashMap<Language, Box<dyn LanguageParser>> = HashMap::new();

        parsers.insert(
            Language::TypeScript,
            Box::new(super::parsers::ts::TypeScriptParser::new()),
        );
        parsers.insert(
            Language::JavaScript,
            Box::new(super::parsers::ts::TypeScriptParser::new()),
        );
        parsers.insert(
            Language::Python,
            Box::new(super::parsers::py::PythonParser::new()),
        );
        parsers.insert(
            Language::Rust,
            Box::new(super::parsers::rust::RustParser::new()),
        );
        parsers.insert(Language::Go, Box::new(super::parsers::go::GoParser::new()));
        parsers.insert(
            Language::Java,
            Box::new(super::parsers::java::JavaParser::new()),
        );
        parsers.insert(
            Language::CSharp,
            Box::new(super::parsers::java::JavaParser::new()),
        );
        parsers.insert(
            Language::Php,
            Box::new(super::parsers::php::PhpParser::new()),
        );

        Self { parsers }
    }

    /// Parse a single file, returning an empty result for unknown languages.
    pub fn parse(&self, file: &SourceFile) -> anyhow::Result<ParseResult> {
        if let Some(parser) = self.parsers.get(&file.language) {
            parser.extract(file)
        } else {
            Ok(ParseResult::new())
        }
    }

    /// Parse all files in parallel using Rayon, logging warnings on individual failures.
    pub fn parse_all(&self, files: &[SourceFile]) -> Vec<ParseResult> {
        use rayon::prelude::*;
        files
            .par_iter()
            .map(|file| {
                self.parse(file).unwrap_or_else(|e| {
                    tracing::warn!("Parse error in {}: {}", file.relative_path, e);
                    ParseResult::new()
                })
            })
            .collect()
    }
}

impl Default for ParserRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Walk back through `item`'s preceding siblings, collecting consecutive comments
/// that pass `is_doc`. Attributes/annotations directly above the item (e.g. Rust
/// `#[derive]`, Java `@Override`, PHP `#[Attr]`) are skipped so the comment search
/// can reach the actual doc block above them.
///
/// Returns the joined doc block in source order, or `None` if no doc comments
/// were found.
pub fn collect_doc_block_above(
    item: tree_sitter::Node,
    source: &[u8],
    is_doc: fn(&str) -> bool,
) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = item.prev_sibling();
    let mut seen_comment = false;
    while let Some(prev) = cur {
        let kind = prev.kind();
        let is_comment = kind == "comment"
            || kind == "line_comment"
            || kind == "block_comment"
            || kind == "doc_comment";
        let is_attribute = !seen_comment
            && matches!(
                kind,
                "attribute_item"
                    | "inner_attribute_item"
                    | "attribute"
                    | "annotation"
                    | "marker_annotation"
                    | "modifiers"
            );

        if is_comment {
            let text = prev.utf8_text(source).unwrap_or("").trim().to_string();
            if !is_doc(&text) {
                break;
            }
            lines.push(text);
            seen_comment = true;
            cur = prev.prev_sibling();
        } else if is_attribute {
            // Skip attributes that sit between the item and its doc block.
            cur = prev.prev_sibling();
        } else {
            break;
        }
    }
    if lines.is_empty() {
        None
    } else {
        lines.reverse();
        Some(lines.join("\n"))
    }
}

/// Walk up `node`'s parents until one of `kinds` is found.
pub fn enclosing_node<'a>(
    node: tree_sitter::Node<'a>,
    kinds: &[&str],
) -> Option<tree_sitter::Node<'a>> {
    let mut cur = Some(node);
    while let Some(n) = cur {
        if kinds.contains(&n.kind()) {
            return Some(n);
        }
        cur = n.parent();
    }
    None
}

/// Set `key` on `node.metadata` (treating it as a JSON object, replacing `Null` with `{}`).
pub fn meta_set(node: &mut NodeDef, key: &str, value: serde_json::Value) {
    if !node.metadata.is_object() {
        node.metadata = serde_json::Value::Object(serde_json::Map::new());
    }
    if let Some(obj) = node.metadata.as_object_mut() {
        obj.insert(key.to_string(), value);
    }
}
