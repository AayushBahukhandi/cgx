use tree_sitter::{Parser, Query, QueryCursor, Node};

use crate::parser::{EdgeDef, EdgeKind, LanguageParser, NodeDef, NodeKind, ParseResult};
use crate::walker::SourceFile;

pub struct TypeScriptParser {
    language: tree_sitter::Language,
}

impl TypeScriptParser {
    pub fn new() -> Self {
        Self {
            language: tree_sitter_typescript::language_typescript(),
        }
    }
}

impl Default for TypeScriptParser {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageParser for TypeScriptParser {
    fn extensions(&self) -> &[&str] {
        &["ts", "tsx", "js", "jsx", "mjs", "cjs"]
    }

    fn extract(&self, file: &SourceFile) -> anyhow::Result<ParseResult> {
        let mut parser = Parser::new();
        parser.set_language(&self.language)?;

        let tree = parser.parse(&file.content, None).ok_or_else(|| {
            anyhow::anyhow!("failed to parse {}", file.relative_path)
        })?;

        let source_bytes = file.content.as_bytes();
        let root = tree.root_node();
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        let fp = file_node_id(&file.relative_path);

        // Parse function declarations
        if let Ok(query) =
            Query::new(&self.language, "(function_declaration name: (identifier) @name) @fn")
        {
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

        // Parse arrow functions / variable declarations with arrow
        if let Ok(query) = Query::new(
            &self.language,
            "(variable_declarator name: (identifier) @name value: (arrow_function) @fn)",
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

        // Parse variable declarations with function expressions
        if let Ok(query) = Query::new(
            &self.language,
            "(variable_declarator name: (identifier) @name value: (function_expression) @fn)",
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
        if let Ok(query) =
            Query::new(&self.language, "(class_declaration name: (type_identifier) @name) @cls")
        {
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

        // Parse method definitions
        if let Ok(query) = Query::new(
            &self.language,
            "(method_definition name: (property_identifier) @name) @m",
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

        // Parse imports — walk the tree directly to find import statements
        extract_imports(&mut edges, root, source_bytes, &fp, file);

        // Parse exports
        if let Ok(query) = Query::new(
            &self.language,
            "(export_statement (function_declaration name: (identifier) @name) @expr)",
        ) {
            process_exports(&mut nodes, &mut edges, file, &query, root, source_bytes, &fp, "fn");
        }

        if let Ok(query) = Query::new(
            &self.language,
            "(export_statement (class_declaration name: (type_identifier) @name) @expr)",
        ) {
            process_exports(&mut nodes, &mut edges, file, &query, root, source_bytes, &fp, "cls");
        }

        // Extract CALLS edges by walking the AST with function context tracking
        extract_calls(&mut edges, root, source_bytes, file);

        Ok(ParseResult { nodes, edges })
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

        let name = unquote_str(&source_bytes[name_capture.node.byte_range()]);
        let node_start = name_capture.node.start_position();

        // Use the body node's end position so the snippet covers the full function/class body
        let body_end = m
            .captures
            .iter()
            .find(|c| {
                let cap_name = &query.capture_names()[c.index as usize];
                *cap_name == "fn" || *cap_name == "cls" || *cap_name == "m"
            })
            .map(|c| c.node.end_position())
            .unwrap_or_else(|| name_capture.node.end_position());

        let Some(_fn_capture) = m
            .captures
            .iter()
            .find(|c| {
                let cap_name = &query.capture_names()[c.index as usize];
                *cap_name == "fn" || *cap_name == "cls" || *cap_name == "m"
            })
        else {
            continue;
        };

        let id = format!("{}:{}:{}", prefix, file.relative_path, name);

        nodes.push(NodeDef {
            id,
            kind: kind.clone(),
            name,
            path: file.relative_path.clone(),
            line_start: node_start.row as u32 + 1,
            line_end: body_end.row as u32 + 1,
            ..Default::default()
        });

        edges.push(EdgeDef {
            src: file_id.to_string(),
            dst: format!("{}:{}:{}", prefix, file.relative_path, unquote_str(
                &source_bytes[name_capture.node.byte_range()]
            )),
            kind: EdgeKind::Exports,
            ..Default::default()
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn process_exports(
    _nodes: &mut Vec<NodeDef>,
    edges: &mut Vec<EdgeDef>,
    file: &SourceFile,
    query: &Query,
    root: tree_sitter::Node,
    source_bytes: &[u8],
    file_id: &str,
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

        edges.push(EdgeDef {
            src: file_id.to_string(),
            dst: format!("{}:{}:{}", prefix, file.relative_path, name),
            kind: EdgeKind::Exports,
            ..Default::default()
        });
    }
}

fn node_text(node: tree_sitter::Node, source: &[u8]) -> String {
    node.utf8_text(source).unwrap_or("").to_string()
}

fn extract_imports(
    edges: &mut Vec<EdgeDef>,
    root: tree_sitter::Node,
    source_bytes: &[u8],
    file_id: &str,
    file: &SourceFile,
) {
    // Walk the entire tree (not just root children) to find imports and requires
    let mut cursor = root.walk();
    traverse_imports(edges, root, source_bytes, file_id, file, &mut cursor);
}

fn traverse_imports(
    edges: &mut Vec<EdgeDef>,
    node: tree_sitter::Node,
    source_bytes: &[u8],
    file_id: &str,
    file: &SourceFile,
    cursor: &mut tree_sitter::TreeCursor,
) {
    if node.kind() == "import_statement" {
        for j in 0..node.child_count() {
            let Some(import_child) = node.child(j) else { continue };
            if import_child.kind() == "string" {
                let import_path = unquote_str(&source_bytes[import_child.byte_range()]);
                if import_path.starts_with('.') {
                    let resolved = resolve_import_path(&file.relative_path, &import_path);
                    if !resolved.is_empty() {
                        edges.push(EdgeDef {
                            src: file_id.to_string(),
                            dst: file_node_id(&resolved),
                            kind: EdgeKind::Imports,
                            ..Default::default()
                        });
                    }
                }
                break;
            }
        }
    } else if node.kind() == "call_expression" {
        // Check for require('...')
        if let Some(func) = node.child_by_field_name("function") {
            if func.kind() == "identifier" && node_text(func, source_bytes) == "require" {
                if let Some(args) = node.child_by_field_name("arguments") {
                    for k in 0..args.child_count() {
                        let Some(arg) = args.child(k) else { continue };
                        if arg.kind() == "string" {
                            let import_path = unquote_str(&source_bytes[arg.byte_range()]);
                            if import_path.starts_with('.') {
                                let resolved = resolve_import_path(&file.relative_path, &import_path);
                                if !resolved.is_empty() {
                                    edges.push(EdgeDef {
                                        src: file_id.to_string(),
                                        dst: file_node_id(&resolved),
                                        kind: EdgeKind::Imports,
                                        ..Default::default()
                                    });
                                }
                            }
                            break;
                        }
                    }
                }
            }
        }
    }

    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            traverse_imports(edges, child, source_bytes, file_id, file, cursor);
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

fn resolve_import_path(current: &str, import: &str) -> String {
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

/// Walk the AST tracking function context, emitting CALLS edges for each call_expression.
fn extract_calls(edges: &mut Vec<EdgeDef>, root: Node, source: &[u8], file: &SourceFile) {
    let mut fn_stack: Vec<String> = Vec::new();
    walk_for_calls(edges, root, source, file, &mut fn_stack);
}

fn is_fn_node(kind: &str) -> bool {
    matches!(kind,
        "function_declaration" | "function" | "arrow_function" |
        "method_definition" | "generator_function_declaration" | "generator_function"
    )
}

fn fn_name_from_node<'a>(node: Node<'a>, source: &[u8], file: &SourceFile) -> Option<String> {
    // function_declaration / generator_function_declaration: has `name` field
    if let Some(name_node) = node.child_by_field_name("name") {
        let name = name_node.utf8_text(source).unwrap_or("").to_string();
        if !name.is_empty() {
            return Some(format!("fn:{}:{}", file.relative_path, name));
        }
    }
    // Arrow/anonymous assigned to variable: look at parent variable_declarator
    let parent = node.parent()?;
    if parent.kind() == "variable_declarator" {
        if let Some(name_node) = parent.child_by_field_name("name") {
            let name = name_node.utf8_text(source).unwrap_or("").to_string();
            if !name.is_empty() {
                return Some(format!("fn:{}:{}", file.relative_path, name));
            }
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
            // anonymous — push a sentinel so pop is balanced
            fn_stack.push(String::new());
        }
    }

    if kind == "call_expression" {
        if let Some(caller_id) = fn_stack.last().filter(|s| !s.is_empty()) {
            let callee_name = node
                .child_by_field_name("function")
                .and_then(|func| match func.kind() {
                    "identifier" => Some(func.utf8_text(source).unwrap_or("").to_string()),
                    "member_expression" => func
                        .child_by_field_name("property")
                        .map(|p| p.utf8_text(source).unwrap_or("").to_string()),
                    _ => None,
                })
                .unwrap_or_default();

            if !callee_name.is_empty() && callee_name != "require" {
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
