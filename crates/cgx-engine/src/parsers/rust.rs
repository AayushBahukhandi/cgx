use tree_sitter::{Parser, Query, QueryCursor};

use crate::parser::{
    collect_doc_block_above, meta_set, EdgeDef, EdgeKind, LanguageParser, NodeDef, NodeKind,
    ParseResult,
};
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
                let fn_node = m
                    .captures
                    .iter()
                    .find(|c| query.capture_names()[c.index as usize] == "fn")
                    .map(|c| c.node);
                let name = node_text(name_capture.node, source_bytes);
                let start = name_capture.node.start_position();
                let body_end = fn_node
                    .map(|n| n.end_position())
                    .unwrap_or_else(|| name_capture.node.end_position());
                let id = format!("fn:{}:{}", file.relative_path, name);

                let doc_comment = fn_node
                    .and_then(|n| collect_doc_block_above(n, source_bytes, is_rust_doc_comment))
                    .map(strip_rust_doc_markers);

                let mut def = NodeDef {
                    id: id.clone(),
                    kind: NodeKind::Function,
                    name,
                    path: file.relative_path.clone(),
                    line_start: start.row as u32 + 1,
                    line_end: body_end.row as u32 + 1,
                    ..Default::default()
                };
                if let Some(doc) = doc_comment {
                    meta_set(&mut def, "doc_comment", serde_json::Value::String(doc));
                }
                nodes.push(def);

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

        // Mark pub items as exported
        mark_pub_exported(&mut nodes, root, source_bytes);

        Ok(ParseResult {
            nodes,
            edges,
            ..Default::default()
        })
    }
}

fn is_pub_item(node: tree_sitter::Node, source_bytes: &[u8]) -> bool {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "visibility_modifier" {
                let text = node_text(child, source_bytes);
                if text == "pub" || text.starts_with("pub(") {
                    return true;
                }
            }
        }
    }
    false
}

fn mark_pub_exported(
    nodes: &mut Vec<crate::parser::NodeDef>,
    root: tree_sitter::Node,
    source_bytes: &[u8],
) {
    walk_pub(nodes, root, source_bytes);
}

fn walk_pub(nodes: &mut Vec<crate::parser::NodeDef>, node: tree_sitter::Node, source_bytes: &[u8]) {
    let kind = node.kind();
    if matches!(
        kind,
        "function_item" | "struct_item" | "enum_item" | "trait_item" | "type_item"
    ) && is_pub_item(node, source_bytes)
    {
        // Get the name of this item
        if let Some(name_node) = node.child_by_field_name("name") {
            let item_name = node_text(name_node, source_bytes);
            // Mark the matching node as exported (preserve any existing metadata, e.g. doc_comment).
            for n in nodes.iter_mut() {
                if n.name == item_name {
                    meta_set(n, "exported", serde_json::Value::Bool(true));
                }
            }
        }
    }

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            walk_pub(nodes, cursor.node(), source_bytes);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

/// True if `text` looks like a Rust doc comment: `///`, `//!`, or `/** */`.
fn is_rust_doc_comment(text: &str) -> bool {
    let t = text.trim_start();
    t.starts_with("///") || t.starts_with("//!") || t.starts_with("/**")
}

/// Strip leading `///`, `//!`, and the `/** ... */` wrapper, joining lines into a single string.
fn strip_rust_doc_markers(raw: String) -> String {
    let mut out: Vec<String> = Vec::new();
    for line in raw.lines() {
        let l = line.trim();
        let stripped = if let Some(rest) = l.strip_prefix("///") {
            rest.trim().to_string()
        } else if let Some(rest) = l.strip_prefix("//!") {
            rest.trim().to_string()
        } else if l.starts_with("/**") {
            l.trim_start_matches("/**")
                .trim_end_matches("*/")
                .trim()
                .to_string()
        } else if l.starts_with("*/") {
            String::new()
        } else if let Some(rest) = l.strip_prefix("*") {
            rest.trim().to_string()
        } else {
            l.to_string()
        };
        out.push(stripped);
    }
    out.join("\n").trim().to_string()
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
        // Use the body/item node for both end position and doc-comment lookup.
        let item_node = m
            .captures
            .iter()
            .find(|c| query.capture_names()[c.index as usize] != "name")
            .map(|c| c.node);
        let body_end = item_node
            .map(|n| n.end_position())
            .unwrap_or_else(|| name_capture.node.end_position());
        let id = format!("{}:{}:{}", prefix, file.relative_path, name);

        let doc_comment = item_node
            .and_then(|n| collect_doc_block_above(n, source_bytes, is_rust_doc_comment))
            .map(strip_rust_doc_markers);

        let mut def = NodeDef {
            id: id.clone(),
            kind: kind.clone(),
            name,
            path: file.relative_path.clone(),
            line_start: start.row as u32 + 1,
            line_end: body_end.row as u32 + 1,
            ..Default::default()
        };
        if let Some(doc) = doc_comment {
            meta_set(&mut def, "doc_comment", serde_json::Value::String(doc));
        }
        nodes.push(def);

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
