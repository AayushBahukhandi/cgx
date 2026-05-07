use std::collections::HashMap;

use crate::walker::{Language, SourceFile};

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum NodeKind {
    File,
    Function,
    Class,
    Variable,
    Type,
    Module,
    Author,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum EdgeKind {
    Calls,
    Imports,
    Inherits,
    Exports,
    CoChanges,
    Owns,
    DependsOn,
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
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NodeDef {
    pub id: String,
    pub kind: NodeKind,
    pub name: String,
    pub path: String,
    pub line_start: u32,
    pub line_end: u32,
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EdgeDef {
    pub src: String,
    pub dst: String,
    pub kind: EdgeKind,
    #[serde(default = "default_edge_weight")]
    pub weight: f64,
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ParseResult {
    pub nodes: Vec<NodeDef>,
    pub edges: Vec<EdgeDef>,
}

impl ParseResult {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }
}

impl Default for ParseResult {
    fn default() -> Self {
        Self::new()
    }
}

pub trait LanguageParser: Send + Sync {
    fn extensions(&self) -> &[&str];
    fn extract(&self, file: &SourceFile) -> anyhow::Result<ParseResult>;
}

pub struct ParserRegistry {
    parsers: HashMap<Language, Box<dyn LanguageParser>>,
}

impl ParserRegistry {
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

    pub fn parse(&self, file: &SourceFile) -> anyhow::Result<ParseResult> {
        if let Some(parser) = self.parsers.get(&file.language) {
            parser.extract(file)
        } else {
            Ok(ParseResult::new())
        }
    }

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
