use tree_sitter::{Node, Parser, Query, QueryCursor};

use crate::parser::{EdgeDef, EdgeKind, LanguageParser, NodeDef, NodeKind, ParseResult};
use crate::walker::SourceFile;

pub struct PhpParser {
    language: tree_sitter::Language,
}

impl PhpParser {
    pub fn new() -> Self {
        Self {
            language: tree_sitter_php::language_php(),
        }
    }
}

impl Default for PhpParser {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageParser for PhpParser {
    fn extensions(&self) -> &[&str] {
        &["php"]
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

        let fp = file_node_id(&file.relative_path);

        // Parse function definitions
        if let Ok(query) = Query::new(
            &self.language,
            "(function_definition name: (name) @name) @fn",
        ) {
            extract_nodes(
                &mut nodes,
                &mut edges,
                file,
                &query,
                root,
                source_bytes,
                NodeKind::Function,
                "fn",
                &fp,
            );
        }

        // Parse class declarations
        if let Ok(query) = Query::new(
            &self.language,
            "(class_declaration name: (name) @name) @cls",
        ) {
            extract_nodes(
                &mut nodes,
                &mut edges,
                file,
                &query,
                root,
                source_bytes,
                NodeKind::Class,
                "cls",
                &fp,
            );
        }

        // Parse interface declarations
        if let Ok(query) = Query::new(
            &self.language,
            "(interface_declaration name: (name) @name) @cls",
        ) {
            extract_nodes(
                &mut nodes,
                &mut edges,
                file,
                &query,
                root,
                source_bytes,
                NodeKind::Class,
                "cls",
                &fp,
            );
        }

        // Parse method declarations
        if let Ok(query) = Query::new(
            &self.language,
            "(method_declaration name: (name) @name) @fn",
        ) {
            extract_nodes(
                &mut nodes,
                &mut edges,
                file,
                &query,
                root,
                source_bytes,
                NodeKind::Function,
                "fn",
                &fp,
            );
        }

        // Parse include/require
        extract_includes(&mut edges, root, source_bytes, &fp, file);

        // Extract calls
        extract_calls(&mut edges, root, source_bytes, file);

        Ok(ParseResult {
            nodes,
            edges,
            ..Default::default()
        })
    }
}

fn file_node_id(rel_path: &str) -> String {
    format!("file:{}", rel_path)
}

#[allow(clippy::too_many_arguments)]
fn extract_nodes(
    nodes: &mut Vec<NodeDef>,
    edges: &mut Vec<EdgeDef>,
    file: &SourceFile,
    query: &Query,
    root: tree_sitter::Node,
    source_bytes: &[u8],
    kind: NodeKind,
    prefix: &str,
    file_id: &str,
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
        let node_start = name_capture.node.start_position();

        let body_end = m
            .captures
            .iter()
            .find(|c| {
                let cap_name = &query.capture_names()[c.index as usize];
                *cap_name == "fn" || *cap_name == "cls"
            })
            .map(|c| c.node.end_position())
            .unwrap_or_else(|| name_capture.node.end_position());

        let id = format!("{}:{}:{}", prefix, file.relative_path, name);

        nodes.push(NodeDef {
            id: id.clone(),
            kind: kind.clone(),
            name: name.clone(),
            path: file.relative_path.clone(),
            line_start: node_start.row as u32 + 1,
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

fn extract_includes(
    edges: &mut Vec<EdgeDef>,
    root: tree_sitter::Node,
    source_bytes: &[u8],
    file_id: &str,
    file: &SourceFile,
) {
    let mut cursor = root.walk();
    traverse_includes(edges, root, source_bytes, file_id, file, &mut cursor);
}

fn traverse_includes(
    edges: &mut Vec<EdgeDef>,
    node: tree_sitter::Node,
    source_bytes: &[u8],
    file_id: &str,
    file: &SourceFile,
    cursor: &mut tree_sitter::TreeCursor,
) {
    // PHP includes: include "file.php", require "file.php", include_once, require_once
    if node.kind() == "include_expression" || node.kind() == "require_expression" {
        for j in 0..node.child_count() {
            let Some(child) = node.child(j) else { continue };
            if child.kind() == "string" {
                let include_path = unquote_str(&source_bytes[child.byte_range()]);
                if !include_path.is_empty() {
                    let resolved = resolve_include_path(&file.relative_path, &include_path);
                    if !resolved.is_empty() {
                        edges.push(EdgeDef {
                            src: file_id.to_string(),
                            dst: file_node_id(&resolved),
                            kind: EdgeKind::Imports,
                            ..Default::default()
                        });
                    }
                }
            }
        }
    }

    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            traverse_includes(edges, child, source_bytes, file_id, file, cursor);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}

fn unquote_str(s: &[u8]) -> String {
    let s = std::str::from_utf8(s).unwrap_or("");
    s.trim().trim_matches('\'').trim_matches('"').to_string()
}

fn resolve_include_path(current: &str, import: &str) -> String {
    let mut parts: Vec<&str> = current.split('/').collect();
    parts.pop(); // remove filename

    for segment in import.split('/') {
        match segment {
            "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(segment),
        }
    }

    parts.join("/")
}

fn extract_calls(edges: &mut Vec<EdgeDef>, root: Node, source: &[u8], file: &SourceFile) {
    let mut fn_stack: Vec<String> = Vec::new();
    walk_for_calls(edges, root, source, file, &mut fn_stack);
}

fn is_fn_node(kind: &str) -> bool {
    matches!(
        kind,
        "function_definition" | "method_declaration" | "anonymous_function_creation_expression"
    )
}

fn fn_name_from_node(node: Node, source: &[u8], file: &SourceFile) -> Option<String> {
    if let Some(name_node) = node.child_by_field_name("name") {
        let name = name_node.utf8_text(source).unwrap_or("").to_string();
        if !name.is_empty() {
            return Some(format!("fn:{}:{}", file.relative_path, name));
        }
    }
    None
}

fn walk_for_calls(
    edges: &mut Vec<EdgeDef>,
    node: Node,
    source: &[u8],
    file: &SourceFile,
    fn_stack: &mut Vec<String>,
) {
    let kind = node.kind();
    let pushed = is_fn_node(kind);

    if pushed {
        if let Some(id) = fn_name_from_node(node, source, file) {
            fn_stack.push(id);
        } else {
            fn_stack.push(String::new());
        }
    }

    if kind == "function_call_expression" {
        if let Some(caller_id) = fn_stack.last().filter(|s| !s.is_empty()) {
            let callee_name = node
                .child_by_field_name("function")
                .and_then(|func| match func.kind() {
                    "name" => Some(func.utf8_text(source).unwrap_or("").to_string()),
                    "qualified_name" => {
                        // Namespace\Class::method or Class::method
                        Some(func.utf8_text(source).unwrap_or("").to_string())
                    }
                    "member_access_expression" => func
                        .child_by_field_name("name")
                        .map(|p| p.utf8_text(source).unwrap_or("").to_string()),
                    _ => None,
                })
                .unwrap_or_default();

            if !callee_name.is_empty() {
                edges.push(EdgeDef {
                    src: caller_id.clone(),
                    dst: callee_name,
                    kind: EdgeKind::Calls,
                    confidence: 0.7,
                    ..Default::default()
                });
            }
        }
    }

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            walk_for_calls(edges, cursor.node(), source, file, fn_stack);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    if pushed {
        fn_stack.pop();
    }
}
