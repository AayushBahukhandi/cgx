use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::process::Command;

use crate::graph::{Edge, GraphDb, Node};

pub fn export_json(db: &GraphDb) -> anyhow::Result<String> {
    let nodes = db.get_all_nodes()?;
    let edges = db.get_all_edges()?;
    let communities = db.get_communities()?;
    let breakdown = db.get_language_breakdown()?;
    let indexed_at = chrono::Utc::now().to_rfc3339();

    let community_list: Vec<serde_json::Value> = communities
        .into_iter()
        .map(|(id, label, node_count, top_nodes)| {
            serde_json::json!({
                "id": id,
                "label": label,
                "node_count": node_count,
                "top_nodes": top_nodes,
            })
        })
        .collect();

    let output = serde_json::json!({
        "meta": {
            "repo_id": db.repo_id,
            "indexed_at": indexed_at,
            "node_count": nodes.len(),
            "edge_count": edges.len(),
            "language_breakdown": breakdown,
            "community_count": community_list.len(),
        },
        "nodes": nodes,
        "edges": edges,
        "communities": community_list,
    });

    Ok(serde_json::to_string_pretty(&output)?)
}

pub fn export_mermaid(db: &GraphDb, max_nodes: usize) -> anyhow::Result<String> {
    let max = max_nodes.clamp(1, 500);
    let nodes = db.get_all_nodes()?;
    let edges = db.get_all_edges()?;

    if nodes.is_empty() {
        return Ok("graph TD\n  A[\"No data\"]\n".to_string());
    }

    let node_ids: HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();

    let mut node_degree: HashMap<String, usize> = HashMap::new();
    for edge in &edges {
        if node_ids.contains(edge.src.as_str()) && node_ids.contains(edge.dst.as_str()) {
            *node_degree.entry(edge.src.clone()).or_default() += 1;
            *node_degree.entry(edge.dst.clone()).or_default() += 1;
        }
    }

    let mut ranked: Vec<&Node> = nodes.iter().collect();
    ranked.sort_by_key(|n| {
        -(node_degree
            .get(&n.id)
            .copied()
            .unwrap_or(0) as i64)
    });
    ranked.truncate(max);

    let selected: HashSet<&str> = ranked.iter().map(|n| n.id.as_str()).collect();

    let mut included_edges: Vec<&Edge> = edges
        .iter()
        .filter(|e| selected.contains(e.src.as_str()) && selected.contains(e.dst.as_str()))
        .collect();
    included_edges.truncate(max);

    let mut output = String::from("graph TD\n");

    let mut safe_ids: HashMap<&str, String> = HashMap::new();
    for node in &ranked {
        let safe = sanitize_mermaid_id(&node.id);
        safe_ids.insert(&node.id, safe.clone());
        let label = sanitize_mermaid_label(&node.name);
        output.push_str(&format!("  {}[\"{}\"]\n", safe, label));
    }

    for edge in &included_edges {
        if let (Some(src_safe), Some(dst_safe)) =
            (safe_ids.get(edge.src.as_str()), safe_ids.get(edge.dst.as_str()))
        {
            let style = match edge.kind.as_str() {
                "CALLS" => "-->",
                "IMPORTS" => "-.->",
                "CO_CHANGES" => "==>",
                _ => "-->",
            };
            output.push_str(&format!("  {} {} {}\n", src_safe, style, dst_safe));
        }
    }

    if ranked.len() < nodes.len() {
        output.push_str(&format!(
            "  %% Showing {}/{} nodes ({} edges filtered)\n",
            ranked.len(),
            nodes.len(),
            edges.len() - included_edges.len()
        ));
    }

    Ok(output)
}

pub fn export_dot(db: &GraphDb) -> anyhow::Result<String> {
    let nodes = db.get_all_nodes()?;
    let edges = db.get_all_edges()?;

    if nodes.is_empty() {
        return Ok("digraph G {\n  label=\"No data\";\n}\n".to_string());
    }

    let mut output = String::from("digraph G {\n");
    output.push_str("  rankdir=LR;\n");
    output.push_str("  bgcolor=\"#0a0a0f\";\n");
    output.push_str("  node [fontname=\"JetBrains Mono\", fontsize=10];\n");
    output.push_str("  edge [fontname=\"JetBrains Mono\", fontsize=8];\n\n");

    let kind_colors: HashMap<&str, &str> = [
        ("Function", "#00ff88"),
        ("Class", "#3b82f6"),
        ("File", "#f59e0b"),
        ("Module", "#8b5cf6"),
        ("Author", "#ec4899"),
        ("Variable", "#06b6d4"),
        ("Type", "#f97316"),
    ]
    .into_iter()
    .collect();

    let max_churn = nodes
        .iter()
        .map(|n| n.churn)
        .fold(0.0f64, f64::max)
        .max(0.01);

    for node in &nodes {
        let safe_id = sanitize_dot_id(&node.id);
        let label = node.name.replace('"', "\\\"");
        let color = kind_colors
            .get(node.kind.as_str())
            .copied()
            .unwrap_or("#888888");
        let size = 0.3 + (node.churn / max_churn) * 0.7;

        output.push_str(&format!(
            "  \"{}\" [label=\"{}\", color=\"{}\", fontcolor=\"{}\", width={:.2}, height={:.2}];\n",
            safe_id, label, color, color, size, size * 0.6
        ));
    }

    output.push('\n');

    let edge_colors: HashMap<&str, &str> = [
        ("CALLS", "#ffffff33"),
        ("IMPORTS", "#3b82f644"),
        ("CO_CHANGES", "#ef444466"),
        ("OWNS", "#ec489944"),
        ("INHERITS", "#8b5cf644"),
    ]
    .into_iter()
    .collect();

    for edge in &edges {
        let safe_src = sanitize_dot_id(&edge.src);
        let safe_dst = sanitize_dot_id(&edge.dst);
        let color = edge_colors
            .get(edge.kind.as_str())
            .copied()
            .unwrap_or("#ffffff22");

        output.push_str(&format!(
            "  \"{}\" -> \"{}\" [color=\"{}\", penwidth={:.1}];\n",
            safe_src,
            safe_dst,
            color,
            (edge.weight * 1.5).clamp(0.5, 5.0)
        ));
    }

    output.push_str("}\n");
    Ok(output)
}

pub fn export_svg(db: &GraphDb) -> anyhow::Result<String> {
    let dot_output = export_dot(db)?;

    if let Ok(svg) = dot_to_svg(&dot_output) {
        return Ok(svg);
    }

    fallback_svg_with_data(db)
}

fn fallback_svg_with_data(db: &GraphDb) -> anyhow::Result<String> {
    let nodes = db.get_all_nodes()?;
    let edges = db.get_all_edges()?;
    render_svg_circle(&nodes, &edges)
}

fn dot_to_svg(dot: &str) -> anyhow::Result<String> {
    let mut child = Command::new("dot")
        .arg("-Tsvg")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(dot.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        anyhow::bail!("dot command failed")
    }
}

fn render_svg_circle(nodes: &[Node], edges: &[Edge]) -> anyhow::Result<String> {

    if nodes.is_empty() {
        return Ok(r##"<svg xmlns="http://www.w3.org/2000/svg" width="400" height="200" viewBox="0 0 400 200">
  <rect width="400" height="200" fill="#0a0a0f"/>
  <text x="200" y="100" text-anchor="middle" fill="#888" font-family="JetBrains Mono, monospace" font-size="14">No data</text>
</svg>"##.to_string());
    }

    let display_nodes: Vec<&Node> = nodes.iter().collect();
    let max_display = display_nodes.len().min(200);
    let display_nodes = &display_nodes[..max_display];

    let node_ids: HashSet<&str> = display_nodes.iter().map(|n| n.id.as_str()).collect();
    let display_edges: Vec<&Edge> = edges
        .iter()
        .filter(|e| node_ids.contains(e.src.as_str()) && node_ids.contains(e.dst.as_str()))
        .take(max_display * 2)
        .collect();

    let center_x = 400.0;
    let center_y = 400.0;
    let radius = 320.0;
    let total = display_nodes.len() as f64;

    let mut positions: HashMap<&str, (f64, f64)> = HashMap::new();
    for (i, node) in display_nodes.iter().enumerate() {
        let angle = 2.0 * std::f64::consts::PI * (i as f64) / total - std::f64::consts::PI / 2.0;
        let x = center_x + radius * angle.cos();
        let y = center_y + radius * angle.sin();
        positions.insert(&node.id, (x, y));
    }

    let kind_colors: HashMap<&str, &str> = [
        ("Function", "#00ff88"),
        ("Class", "#3b82f6"),
        ("File", "#f59e0b"),
        ("Module", "#8b5cf6"),
        ("Author", "#ec4899"),
        ("Variable", "#06b6d4"),
        ("Type", "#f97316"),
    ]
    .into_iter()
    .collect();

    let edge_colors: HashMap<&str, &str> = [
        ("CALLS", "#ffffff22"),
        ("IMPORTS", "#3b82f644"),
        ("CO_CHANGES", "#ef444466"),
        ("OWNS", "#ec489944"),
        ("INHERITS", "#8b5cf644"),
    ]
    .into_iter()
    .collect();

    let mut svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="800" height="800" viewBox="0 0 800 800">
  <rect width="800" height="800" fill="#0a0a0f"/>
  <text x="10" y="20" fill="#555" font-family="JetBrains Mono, monospace" font-size="10">cgx graph - {} nodes, {} edges</text>
"##,
        nodes.len(),
        edges.len()
    );

    for edge in &display_edges {
        if let (Some(&(x1, y1)), Some(&(x2, y2))) =
            (positions.get(edge.src.as_str()), positions.get(edge.dst.as_str()))
        {
            let color = edge_colors
                .get(edge.kind.as_str())
                .copied()
                .unwrap_or("#ffffff11");
            svg.push_str(&format!(
                r##"  <line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="{}" stroke-width="{:.1}" opacity="0.6"/>
"##,
                x1, y1, x2, y2, color, (edge.weight * 1.0).clamp(0.3, 3.0)
            ));
        }
    }

    for node in display_nodes.iter() {
        if let Some(&(x, y)) = positions.get(node.id.as_str()) {
            let color = kind_colors
                .get(node.kind.as_str())
                .copied()
                .unwrap_or("#888888");
            let r = 4.0 + (node.churn * 8.0).min(12.0);
            let label = node.name.chars().take(20).collect::<String>();
            let escaped_label = label.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");

            svg.push_str(&format!(
                r##"  <circle cx="{:.1}" cy="{:.1}" r="{:.1}" fill="{}" opacity="0.8"/>
"##,
                x, y, r, color
            ));
            svg.push_str(&format!(
                r##"  <text x="{:.1}" y="{:.1}" fill="#ccc" font-family="JetBrains Mono, monospace" font-size="8" text-anchor="middle">{}</text>
"##,
                x,
                y - r - 3.0,
                escaped_label
            ));
        }
    }

    svg.push_str("</svg>\n");
    Ok(svg)
}

pub fn export_graphml(db: &GraphDb) -> anyhow::Result<String> {
    let nodes = db.get_all_nodes()?;
    let edges = db.get_all_edges()?;

    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<graphml xmlns="http://graphml.graphdrawing.org/xmlns"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://graphml.graphdrawing.org/xmlns
         http://graphml.graphdrawing.org/xmlns/1.0/graphml.xsd">
  <key id="name" for="node" attr.name="name" attr.type="string"/>
  <key id="kind" for="node" attr.name="kind" attr.type="string"/>
  <key id="path" for="node" attr.name="path" attr.type="string"/>
  <key id="churn" for="node" attr.name="churn" attr.type="double"/>
  <key id="coupling" for="node" attr.name="coupling" attr.type="double"/>
  <key id="community" for="node" attr.name="community" attr.type="long"/>
  <key id="language" for="node" attr.name="language" attr.type="string"/>
  <key id="kind" for="edge" attr.name="kind" attr.type="string"/>
  <key id="weight" for="edge" attr.name="weight" attr.type="double"/>
  <key id="confidence" for="edge" attr.name="confidence" attr.type="double"/>
  <graph id="G" edgedefault="directed">
"#,
    );

    for node in &nodes {
        let safe_id = xml_escape(&node.id);
        let safe_name = xml_escape(&node.name);
        let safe_path = xml_escape(&node.path);
        let safe_kind = xml_escape(&node.kind);
        let safe_lang = xml_escape(&node.language);

        xml.push_str(&format!(
            "    <node id=\"{}\">\n",
            safe_id
        ));
        xml.push_str(&format!(
            "      <data key=\"name\">{}</data>\n",
            safe_name
        ));
        xml.push_str(&format!(
            "      <data key=\"kind\">{}</data>\n",
            safe_kind
        ));
        xml.push_str(&format!(
            "      <data key=\"path\">{}</data>\n",
            safe_path
        ));
        xml.push_str(&format!(
            "      <data key=\"churn\">{}</data>\n",
            node.churn
        ));
        xml.push_str(&format!(
            "      <data key=\"coupling\">{}</data>\n",
            node.coupling
        ));
        xml.push_str(&format!(
            "      <data key=\"community\">{}</data>\n",
            node.community
        ));
        xml.push_str(&format!(
            "      <data key=\"language\">{}</data>\n",
            safe_lang
        ));
        xml.push_str("    </node>\n");
    }

    for edge in &edges {
        let safe_src = xml_escape(&edge.src);
        let safe_dst = xml_escape(&edge.dst);
        let safe_kind = xml_escape(&edge.kind);

        xml.push_str(&format!(
            "    <edge source=\"{}\" target=\"{}\">\n",
            safe_src, safe_dst
        ));
        xml.push_str(&format!(
            "      <data key=\"kind\">{}</data>\n",
            safe_kind
        ));
        xml.push_str(&format!(
            "      <data key=\"weight\">{}</data>\n",
            edge.weight
        ));
        xml.push_str(&format!(
            "      <data key=\"confidence\">{}</data>\n",
            edge.confidence
        ));
        xml.push_str("    </edge>\n");
    }

    xml.push_str("  </graph>\n</graphml>\n");
    Ok(xml)
}

fn sanitize_mermaid_id(id: &str) -> String {
    id.replace([':', '.', '/', '-', '(', ')', '[', ']', ' ', '<', '>', '|'], "_")
}

fn sanitize_mermaid_label(name: &str) -> String {
    name.replace('"', "'")
        .replace('[', "(")
        .replace(']', ")")
        .chars()
        .take(40)
        .collect()
}

fn sanitize_dot_id(id: &str) -> String {
    id.replace('"', "\\\"")
        .replace(['\n', '\r'], " ")
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
