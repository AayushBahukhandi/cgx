use tree_sitter::{Parser, Query, QueryCursor};

use crate::parser::{EdgeDef, EdgeKind, LanguageParser, NodeDef, NodeKind, ParseResult};
use crate::walker::SourceFile;

pub struct RustParser {
    language: tree_sitter::Language,
}

impl RustParser {
    pub fn new() -> Self {
        Self {
            language: tree_sitter_rust::language(),
        }
    }
}

impl Default for RustParser {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageParser for RustParser {
    fn extensions(&self) -> &[&str] {
        &["rs"]
    }

    fn extract(&self, file: &SourceFile) -> anyhow::Result<ParseResult> {
        let mut parser = Parser::new();
        parser.set_language(&self.language)?;

        let tree = parser
            .parse(&file.content, None)
            .ok_or_else(|| anyhow::anyhow!("failed to parse {}", file.relative_path))?;

        let source_bytes = file.content.as_bytes();
        let root = tree.root_node();
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        let fp = format!("file:{}", file.relative_path);

        // Function definitions
        if let Ok(query) = Query::new(
            &self.language,
            "(function_item name: (identifier) @name) @fn",
        ) {
            let mut cursor = QueryCursor::new();
            for m in cursor.matches(&query, root, source_bytes) {
                let Some(name_capture) = m
                    .captures
                    .iter()
                    .find(|c| query.capture_names()[c.index as usize] == "name")
                else {
                    continue;
                };
                let name = node_text(name_capture.node, source_bytes);
                let start = name_capture.node.start_position();
                let body_end = m
                    .captures
                    .iter()
                    .find(|c| query.capture_names()[c.index as usize] == "fn")
                    .map(|c| c.node.end_position())
                    .unwrap_or_else(|| name_capture.node.end_position());
                let id = format!("fn:{}:{}", file.relative_path, name);

                nodes.push(NodeDef {
                    id: id.clone(),
                    kind: NodeKind::Function,
                    name,
                    path: file.relative_path.clone(),
                    line_start: start.row as u32 + 1,
                    line_end: body_end.row as u32 + 1,
                    ..Default::default()
                });

                edges.push(EdgeDef {
                    src: fp.clone(),
                    dst: id,
                    kind: EdgeKind::Exports,
                    ..Default::default()
                });
            }
        }

        // Struct definitions
        if let Ok(query) = Query::new(
            &self.language,
            "(struct_item name: (type_identifier) @name) @s",
        ) {
            extract_type_nodes(
                &mut nodes,
                &mut edges,
                &fp,
                file,
                &query,
                root,
                source_bytes,
                NodeKind::Class,
                "cls",
            );
        }

        // Enum definitions
        if let Ok(query) = Query::new(
            &self.language,
            "(enum_item name: (type_identifier) @name) @e",
        ) {
            extract_type_nodes(
                &mut nodes,
                &mut edges,
                &fp,
                file,
                &query,
                root,
                source_bytes,
                NodeKind::Class,
                "cls",
            );
        }

        // Trait definitions
        if let Ok(query) = Query::new(
            &self.language,
            "(trait_item name: (type_identifier) @name) @t",
        ) {
            extract_type_nodes(
                &mut nodes,
                &mut edges,
                &fp,
                file,
                &query,
                root,
                source_bytes,
                NodeKind::Class,
                "cls",
            );
        }

        // Impl blocks — add edges for impl'd struct/trait methods
        if let Ok(query) = Query::new(
            &self.language,
            "(impl_item type: (type_identifier) @type body: (_) @body)",
        ) {
            let mut cursor = QueryCursor::new();
            for m in cursor.matches(&query, root, source_bytes) {
                if let Some(type_cap) = m
                    .captures
                    .iter()
                    .find(|c| query.capture_names()[c.index as usize] == "type")
                {
                    let type_name = node_text(type_cap.node, source_bytes);
                    edges.push(EdgeDef {
                        src: fp.clone(),
                        dst: format!("cls:{}:{}", file.relative_path, type_name),
                        kind: EdgeKind::Exports,
                        ..Default::default()
                    });
                }
            }
        }

        // Use statements
        if let Ok(query) = Query::new(
            &self.language,
            "(use_declaration argument: (scoped_identifier path: (_) @path name: (_)?))",
        ) {
            let mut cursor = QueryCursor::new();
            for m in cursor.matches(&query, root, source_bytes) {
                if let Some(path_cap) = m
                    .captures
                    .iter()
                    .find(|c| query.capture_names()[c.index as usize] == "path")
                {
                    let full_path = node_text(path_cap.node, source_bytes);
                    // Simple case: use crate::foo::bar -> file path is src/foo/bar.rs
                    let import_path = if full_path.starts_with("crate::") {
                        format!(
                            "src/{}.rs",
                            full_path.trim_start_matches("crate::").replace("::", "/")
                        )
                    } else {
                        continue;
                    };
                    edges.push(EdgeDef {
                        src: fp.clone(),
                        dst: format!("file:{}", import_path),
                        kind: EdgeKind::Imports,
                        ..Default::default()
                    });
                }
            }
        }

        // Simpler use declarations (use foo::Bar)
        if let Ok(query) = Query::new(
            &self.language,
            "(use_declaration argument: (identifier) @name)",
        ) {
            let mut cursor = QueryCursor::new();
            for m in cursor.matches(&query, root, source_bytes) {
                if let Some(name_cap) = m
                    .captures
                    .iter()
                    .find(|c| query.capture_names()[c.index as usize] == "name")
                {
                    let mod_name = node_text(name_cap.node, source_bytes);
                    let import_path = mod_name;
                    edges.push(EdgeDef {
                        src: fp.clone(),
                        dst: format!("file:{}.rs", import_path),
                        kind: EdgeKind::Imports,
                        ..Default::default()
                    });
                }
            }
        }

        Ok(ParseResult {
            nodes,
            edges,
            ..Default::default()
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn extract_type_nodes(
    nodes: &mut Vec<NodeDef>,
    edges: &mut Vec<EdgeDef>,
    file_id: &str,
    file: &SourceFile,
    query: &Query,
    root: tree_sitter::Node,
    source_bytes: &[u8],
    kind: NodeKind,
    prefix: &str,
) {
    let mut cursor = QueryCursor::new();
    for m in cursor.matches(query, root, source_bytes) {
        let Some(name_capture) = m
            .captures
            .iter()
            .find(|c| query.capture_names()[c.index as usize] == "name")
        else {
            continue;
        };
        let name = node_text(name_capture.node, source_bytes);
        let start = name_capture.node.start_position();
        // Use the body node for end position; fall back to name node if no body capture
        let body_end = m
            .captures
            .iter()
            .find(|c| query.capture_names()[c.index as usize] != "name")
            .map(|c| c.node.end_position())
            .unwrap_or_else(|| name_capture.node.end_position());
        let id = format!("{}:{}:{}", prefix, file.relative_path, name);

        nodes.push(NodeDef {
            id: id.clone(),
            kind: kind.clone(),
            name,
            path: file.relative_path.clone(),
            line_start: start.row as u32 + 1,
            line_end: body_end.row as u32 + 1,
            ..Default::default()
        });

        edges.push(EdgeDef {
            src: file_id.to_string(),
            dst: id,
            kind: EdgeKind::Exports,
            ..Default::default()
        });
    }
}

fn node_text(node: tree_sitter::Node, source: &[u8]) -> String {
    node.utf8_text(source).unwrap_or("").to_string()
}
