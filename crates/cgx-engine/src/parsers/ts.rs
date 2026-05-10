use tree_sitter::{Node, Parser, Query, QueryCursor};

use crate::parser::{
    CommentKind, CommentTag, EdgeDef, EdgeKind, LanguageParser, NodeDef, NodeKind, ParseResult,
};
use crate::walker::SourceFile;

pub struct TypeScriptParser;

impl TypeScriptParser {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TypeScriptParser {
    fn default() -> Self {
        Self::new()
    }
}

fn is_jsx_extension(path: &str) -> bool {
    path.ends_with(".tsx") || path.ends_with(".jsx")
}

impl LanguageParser for TypeScriptParser {
    fn extensions(&self) -> &[&str] {
        &["ts", "tsx", "js", "jsx", "mjs", "cjs"]
    }

    fn extract(&self, file: &SourceFile) -> anyhow::Result<ParseResult> {
        // TSX/JSX files must use the TSX grammar; TypeScript grammar rejects JSX syntax
        // and produces error nodes with wrong line positions for every JSX element.
        let language = if is_jsx_extension(&file.relative_path) {
            tree_sitter_typescript::language_tsx()
        } else {
            tree_sitter_typescript::language_typescript()
        };

        let mut parser = Parser::new();
        parser.set_language(&language)?;

        let tree = parser
            .parse(&file.content, None)
            .ok_or_else(|| anyhow::anyhow!("failed to parse {}", file.relative_path))?;

        let source_bytes = file.content.as_bytes();
        let root = tree.root_node();
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        let fp = file_node_id(&file.relative_path);

        // Parse function declarations
        if let Ok(query) = Query::new(
            &language,
            "(function_declaration name: (identifier) @name) @fn",
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

        // Parse arrow functions / variable declarations with arrow
        if let Ok(query) = Query::new(
            &language,
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
            &language,
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
        if let Ok(query) = Query::new(
            &language,
            "(class_declaration name: (type_identifier) @name) @cls",
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

        // Parse method definitions
        if let Ok(query) = Query::new(
            &language,
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
            &language,
            "(export_statement (function_declaration name: (identifier) @name) @expr)",
        ) {
            process_exports(
                &mut nodes,
                &mut edges,
                file,
                &query,
                root,
                source_bytes,
                &fp,
                "fn",
            );
        }

        if let Ok(query) = Query::new(
            &language,
            "(export_statement (class_declaration name: (type_identifier) @name) @expr)",
        ) {
            process_exports(
                &mut nodes,
                &mut edges,
                file,
                &query,
                root,
                source_bytes,
                &fp,
                "cls",
            );
        }

        // Extract CALLS edges by walking the AST with function context tracking
        extract_calls(&mut edges, root, source_bytes, file);

        // Mark exported nodes based on export statements
        // Merge exported=true into existing metadata (preserving complexity, doc_comment, etc.)
        let exported_names = collect_exported_names(root, source_bytes);
        for node in &mut nodes {
            if exported_names.contains(&node.name) {
                if let Some(obj) = node.metadata.as_object_mut() {
                    obj.insert("exported".to_string(), serde_json::Value::Bool(true));
                } else {
                    node.metadata = serde_json::json!({"exported": true});
                }
            }
        }

        // Extract JSX expression comments and annotation tags (TODO/FIXME/etc.)
        let mut comment_tags = Vec::new();
        extract_jsx_comments(&mut comment_tags, root, source_bytes, false);

        Ok(ParseResult {
            nodes,
            edges,
            comment_tags,
        })
    }
}

fn collect_exported_names(
    root: tree_sitter::Node,
    source_bytes: &[u8],
) -> std::collections::HashSet<String> {
    let mut exported = std::collections::HashSet::new();
    collect_exported_names_walk(root, source_bytes, &mut exported);
    exported
}

fn collect_exported_names_walk(
    node: tree_sitter::Node,
    source_bytes: &[u8],
    exported: &mut std::collections::HashSet<String>,
) {
    if node.kind() == "export_statement" {
        // Walk children to find identifiers/function names
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                match child.kind() {
                    "function_declaration" | "class_declaration" => {
                        if let Some(name_node) = child.child_by_field_name("name") {
                            exported.insert(node_text(name_node, source_bytes));
                        }
                    }
                    "variable_declaration" => {
                        // export const foo = ...
                        for j in 0..child.child_count() {
                            if let Some(decl) = child.child(j) {
                                if decl.kind() == "variable_declarator" {
                                    if let Some(name_node) = decl.child_by_field_name("name") {
                                        exported.insert(node_text(name_node, source_bytes));
                                    }
                                }
                            }
                        }
                    }
                    "export_clause" => {
                        // export { foo, bar }
                        for j in 0..child.child_count() {
                            if let Some(spec) = child.child(j) {
                                if spec.kind() == "export_specifier" {
                                    if let Some(name_node) = spec.child_by_field_name("name") {
                                        exported.insert(node_text(name_node, source_bytes));
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    // recurse
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            collect_exported_names_walk(child, source_bytes, exported);
        }
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

        let fn_capture_node = m.captures.iter().find(|c| {
            let cap_name = &query.capture_names()[c.index as usize];
            *cap_name == "fn" || *cap_name == "cls" || *cap_name == "m"
        });

        let Some(fn_capture) = fn_capture_node else {
            continue;
        };

        let id = format!("{}:{}:{}", prefix, file.relative_path, name);

        // Compute cyclomatic-style complexity
        let complexity = compute_complexity(fn_capture.node, source_bytes);

        // Extract doc comment (/** */ or /// just before the function)
        let fn_line = node_start.row as u32 + 1;
        let doc_comment = extract_doc_comment(root, source_bytes, fn_line);

        let metadata = serde_json::json!({
            "complexity": complexity,
            "doc_comment": doc_comment,
        });

        nodes.push(NodeDef {
            id,
            kind: kind.clone(),
            name,
            path: file.relative_path.clone(),
            line_start: fn_line,
            line_end: body_end.row as u32 + 1,
            metadata,
        });

        edges.push(EdgeDef {
            src: file_id.to_string(),
            dst: format!(
                "{}:{}:{}",
                prefix,
                file.relative_path,
                unquote_str(&source_bytes[name_capture.node.byte_range()])
            ),
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
            let Some(import_child) = node.child(j) else {
                continue;
            };
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
                                let resolved =
                                    resolve_import_path(&file.relative_path, &import_path);
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
    matches!(
        kind,
        "function_declaration"
            | "function"
            | "arrow_function"
            | "method_definition"
            | "generator_function_declaration"
            | "generator_function"
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

    // Effective caller: innermost named function, or the file node for module-level code
    let caller_id: Option<String> = fn_stack
        .iter()
        .rev()
        .find(|s| !s.is_empty())
        .cloned()
        .or_else(|| Some(format!("file:{}", file.relative_path)));

    if kind == "call_expression" {
        if let Some(ref caller) = caller_id {
            let func_node = node.child_by_field_name("function");
            let callee_name = func_node
                .as_ref()
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
                    src: caller.clone(),
                    dst: callee_name,
                    kind: EdgeKind::Calls,
                    confidence: 0.7,
                    ..Default::default()
                });
            }

            // For `Obj.method()` also emit a CALLS edge to the object identifier so
            // classes used only via static methods aren't flagged as dead code.
            if let Some(func) = func_node {
                if func.kind() == "member_expression" {
                    if let Some(obj) = func.child_by_field_name("object") {
                        if obj.kind() == "identifier" {
                            let obj_name = obj.utf8_text(source).unwrap_or("").to_string();
                            if !obj_name.is_empty() {
                                edges.push(EdgeDef {
                                    src: caller.clone(),
                                    dst: obj_name,
                                    kind: EdgeKind::Calls,
                                    confidence: 0.6,
                                    ..Default::default()
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // new_expression: `new ClassName(...)` — emit CALLS edge to the constructor
    if kind == "new_expression" {
        if let Some(ref caller) = caller_id {
            let constructor_name = node
                .child_by_field_name("constructor")
                .and_then(|c| match c.kind() {
                    "identifier" => Some(c.utf8_text(source).unwrap_or("").to_string()),
                    "member_expression" => c
                        .child_by_field_name("property")
                        .map(|p| p.utf8_text(source).unwrap_or("").to_string()),
                    _ => None,
                })
                .unwrap_or_default();

            if !constructor_name.is_empty() {
                edges.push(EdgeDef {
                    src: caller.clone(),
                    dst: constructor_name,
                    kind: EdgeKind::Calls,
                    confidence: 0.7,
                    ..Default::default()
                });
            }
        }
    }

    // JSX component usage: <ComponentName ... /> and <ComponentName ...>
    // Treat JSX elements as calls from the enclosing function to the component.
    if kind == "jsx_opening_element" || kind == "jsx_self_closing_element" {
        if let Some(ref caller_id) = caller_id {
            let tag_name = node
                .child_by_field_name("name")
                .map(|n| n.utf8_text(source).unwrap_or("").to_string())
                .unwrap_or_default();

            // Only emit edges for PascalCase or camelCase user-defined components.
            // Lowercase tags like <div>, <span> are HTML intrinsics — skip them.
            let is_component = tag_name
                .chars()
                .next()
                .map(|c| {
                    c.is_uppercase()
                        || (c.is_lowercase() && tag_name.len() > 3 && tag_name.contains('.'))
                })
                .unwrap_or(false);

            if is_component {
                // Strip member access for <Namespace.Component /> — use only the last segment
                let callee = tag_name
                    .split('.')
                    .next_back()
                    .unwrap_or(&tag_name)
                    .to_string();
                edges.push(EdgeDef {
                    src: caller_id.clone(),
                    dst: callee,
                    kind: EdgeKind::Calls,
                    confidence: 0.6,
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

/// Compute a normalized cyclomatic-style complexity score for a function node.
/// Returns a value in 0.0..=1.0 (raw score capped at 100, then divided by 100).
fn compute_complexity(node: tree_sitter::Node, source: &[u8]) -> f64 {
    let raw = count_complexity(node, source, 0);
    let capped = raw.min(100.0);
    capped / 100.0
}

fn count_complexity(node: tree_sitter::Node, source: &[u8], nesting: u32) -> f64 {
    let mut score: f64 = 0.0;
    let kind = node.kind();

    let is_branching = matches!(
        kind,
        "if_statement"
            | "for_statement"
            | "for_in_statement"
            | "while_statement"
            | "do_statement"
            | "switch_statement"
            | "catch_clause"
            | "ternary_expression"
    );

    if is_branching {
        score += 1.0 + (nesting as f64 * 0.5);
    }

    // logical expressions (&&, ||)
    if kind == "binary_expression" || kind == "logical_expression" {
        if let Some(op) = node.child_by_field_name("operator") {
            let op_text = op.utf8_text(source).unwrap_or("");
            if op_text == "&&" || op_text == "||" {
                score += 0.5;
            }
        }
    }

    let new_nesting = if is_branching { nesting + 1 } else { nesting };

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            score += count_complexity(cursor.node(), source, new_nesting);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    score
}

/// Look for a doc comment (/** */ or ///) just before the function line.
/// Searches up to 3 lines before fn_line for a comment node ending near that line.
fn extract_doc_comment(
    root: tree_sitter::Node,
    source: &[u8],
    fn_line: u32,
) -> Option<String> {
    find_doc_comment(root, source, fn_line)
}

fn find_doc_comment(
    node: tree_sitter::Node,
    source: &[u8],
    fn_line: u32,
) -> Option<String> {
    if node.kind() == "comment" {
        let end_line = node.end_position().row as u32 + 1;
        // Comment should end at or just before the function line (within 3 lines)
        if end_line >= fn_line.saturating_sub(3) && end_line < fn_line {
            let text = node.utf8_text(source).unwrap_or("").trim().to_string();
            if text.starts_with("/**") || text.starts_with("///") {
                return Some(text);
            }
        }
    }

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            if let Some(result) = find_doc_comment(cursor.node(), source, fn_line) {
                return Some(result);
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    None
}

const ANNOTATION_TAGS: &[&str] = &[
    "TODO", "FIXME", "HACK", "NOTE", "BUG", "OPTIMIZE", "WARN", "XXX",
];

/// Recursively walk the AST extracting annotation comments (TODO/FIXME/etc.).
/// `in_jsx_expression` tracks whether we are inside a `jsx_expression` node,
/// which is how `{/* ... */}` comments appear in the TSX grammar.
fn extract_jsx_comments(
    tags: &mut Vec<CommentTag>,
    node: Node,
    source: &[u8],
    in_jsx_expression: bool,
) {
    let kind = node.kind();

    // Track whether we're entering a jsx_expression wrapper
    let now_in_jsx = in_jsx_expression || kind == "jsx_expression";

    if kind == "comment" {
        let raw = node.utf8_text(source).unwrap_or("").trim();

        let comment_kind = if in_jsx_expression {
            // Strip `/*` / `*/` delimiters and check for commented-out JSX code
            let inner = raw.trim_start_matches("/*").trim_end_matches("*/").trim();
            if inner.starts_with('<') || inner.contains("</") || inner.contains("/>") {
                CommentKind::JsxCommentedCode
            } else {
                CommentKind::JsxExpression
            }
        } else {
            CommentKind::Standard
        };

        let upper = raw.to_uppercase();
        for &tag in ANNOTATION_TAGS {
            if upper.contains(tag) {
                tags.push(CommentTag {
                    tag_type: tag.to_string(),
                    text: raw.to_string(),
                    line: node.start_position().row as u32 + 1,
                    comment_kind: comment_kind.clone(),
                });
                break;
            }
        }
    }

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            extract_jsx_comments(tags, cursor.node(), source, now_in_jsx);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}
