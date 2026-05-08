use tree_sitter::{Parser, Query, QueryCursor};

use crate::parser::{EdgeDef, EdgeKind, LanguageParser, NodeDef, NodeKind, ParseResult};
use crate::walker::SourceFile;

pub struct PythonParser {
    language: tree_sitter::Language,
}

impl PythonParser {
    pub fn new() -> Self {
        Self {
            language: tree_sitter_python::language(),
        }
    }
}

impl Default for PythonParser {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageParser for PythonParser {
    fn extensions(&self) -> &[&str] {
        &["py"]
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
            "(function_definition name: (identifier) @name) @fn",
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

        // Class definitions
        if let Ok(query) = Query::new(
            &self.language,
            "(class_definition name: (identifier) @name) @cls",
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
                    .find(|c| query.capture_names()[c.index as usize] == "cls")
                    .map(|c| c.node.end_position())
                    .unwrap_or_else(|| name_capture.node.end_position());
                let id = format!("cls:{}:{}", file.relative_path, name);

                nodes.push(NodeDef {
                    id: id.clone(),
                    kind: NodeKind::Class,
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

        // Import statements: `from X import Y`
        if let Ok(query) = Query::new(
            &self.language,
            r#"(import_from_statement
                module_name: (dotted_name) @mod
                name: (dotted_name (identifier) @name))
            "#,
        ) {
            let mut cursor = QueryCursor::new();
            for m in cursor.matches(&query, root, source_bytes) {
                let mod_name = m
                    .captures
                    .iter()
                    .find(|c| query.capture_names()[c.index as usize] == "mod")
                    .map(|c| node_text(c.node, source_bytes));
                let import_name = m
                    .captures
                    .iter()
                    .find(|c| query.capture_names()[c.index as usize] == "name")
                    .map(|c| node_text(c.node, source_bytes));

                if let (Some(mod_name), Some(_import_name)) = (mod_name, import_name) {
                    let import_path = resolve_py_import(&file.relative_path, &mod_name);
                    edges.push(EdgeDef {
                        src: fp.clone(),
                        dst: format!("file:{}", import_path),
                        kind: EdgeKind::Imports,
                        ..Default::default()
                    });
                }
            }
        }

        // Import statements: `import X`
        if let Ok(query) = Query::new(
            &self.language,
            "(import_statement name: (dotted_name (identifier) @name))",
        ) {
            let mut cursor = QueryCursor::new();
            for m in cursor.matches(&query, root, source_bytes) {
                if let Some(cap) = m
                    .captures
                    .iter()
                    .find(|c| query.capture_names()[c.index as usize] == "name")
                {
                    let mod_name = node_text(cap.node, source_bytes);
                    let import_path = format!("{}.py", mod_name.replace('.', "/"));
                    edges.push(EdgeDef {
                        src: fp.clone(),
                        dst: format!("file:{}", import_path),
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

fn node_text(node: tree_sitter::Node, source: &[u8]) -> String {
    node.utf8_text(source).unwrap_or("").to_string()
}

fn resolve_py_import(current_file: &str, module_name: &str) -> String {
    let dot_count = module_name.chars().take_while(|c| *c == '.').count();

    if dot_count > 0 {
        // Relative import: `.models` stays in same dir, `..models` goes up 1, etc.
        let remainder = &module_name[dot_count..];
        let mut parts: Vec<&str> = current_file.split('/').collect();
        parts.pop(); // remove filename
                     // `.` = 0 extra pops, `..` = 1 extra pop, `...` = 2 extra pops
        let up_count = dot_count.saturating_sub(1);
        for _ in 0..up_count {
            parts.pop();
        }
        if remainder.is_empty() {
            // `from . import X` — importing from current package's __init__.py
            parts.push("__init__");
        } else {
            parts.push(remainder);
        }
        format!("{}.py", parts.join("/"))
    } else {
        // Absolute import
        format!("{}.py", module_name.replace('.', "/"))
    }
}
