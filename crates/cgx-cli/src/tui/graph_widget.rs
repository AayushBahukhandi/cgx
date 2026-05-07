use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{
        canvas::{Canvas, Line as CanvasLine},
        Widget,
    },
};

use super::app::App;

pub struct GraphWidget;

impl GraphWidget {
    pub fn node_color(kind: &str) -> Color {
        match kind {
            "Function" => Color::Rgb(0, 255, 136),
            "Class"    => Color::Rgb(59, 130, 246),
            "File"     => Color::Rgb(245, 158, 11),
            "Module"   => Color::Rgb(139, 92, 246),
            "Variable" => Color::Rgb(52, 211, 153),
            "Type"     => Color::Rgb(168, 85, 247),
            "Author"   => Color::Rgb(236, 72, 153),
            _          => Color::Gray,
        }
    }

    pub fn node_glyph(kind: &str) -> &'static str {
        match kind {
            "Function" => "◉",
            "Class"    => "◈",
            "File"     => "◧",
            "Module"   => "⬡",
            "Variable" => "◦",
            "Type"     => "◇",
            "Author"   => "◎",
            _          => "●",
        }
    }
}

pub fn render_graph(app: &App, area: Rect, buf: &mut Buffer) {
    if app.visible_node_count() == 0 {
        let msg = if app.nodes.is_empty() {
            "No graph indexed. Run `cgx analyze` first."
        } else {
            "No matching nodes (try adjusting filters)."
        };
        let x = area.x + area.width.saturating_sub(msg.len() as u16) / 2;
        let y = area.y + area.height / 2;
        buf.set_string(x, y, msg, Style::default().fg(Color::Gray));
        return;
    }

    const ORIG_W: f64 = 200.0;
    const ORIG_H: f64 = 160.0;

    // Zoom/pan: scale around (ORIG_W/2 + pan_x, ORIG_H/2 + pan_y).
    // At zoom=1 / pan=0 this is the exact identity — vp(x,y) == (x,y).
    let zoom = app.zoom;
    let pan_x = app.pan_x;
    let pan_y = app.pan_y;
    let vp = |gx: f64, gy: f64| -> (f64, f64) {
        (
            (gx - ORIG_W / 2.0 - pan_x) * zoom + ORIG_W / 2.0,
            (gy - ORIG_H / 2.0 - pan_y) * zoom + ORIG_H / 2.0,
        )
    };

    // Collect viewport-transformed edge endpoints for the canvas
    let mut edge_lines: Vec<(f64, f64, f64, f64)> = Vec::new();
    for edge in app.visible_edges_for_display() {
        if let (Some(&(gx1, gy1)), Some(&(gx2, gy2))) =
            (app.positions.get(&edge.src), app.positions.get(&edge.dst))
        {
            let (vx1, vy1) = vp(gx1, gy1);
            let (vx2, vy2) = vp(gx2, gy2);
            edge_lines.push((vx1, vy1, vx2, vy2));
        }
    }

    // Canvas still uses the original fixed bounds; vp() moves positions into/out of them.
    Canvas::default()
        .x_bounds([0.0, ORIG_W])
        .y_bounds([0.0, ORIG_H])
        .paint(move |ctx| {
            for &(x1, y1, x2, y2) in &edge_lines {
                ctx.draw(&CanvasLine { x1, y1, x2, y2, color: Color::Rgb(70, 70, 100) });
            }
        })
        .render(area, buf);

    // Original to_screen — unchanged from pre-zoom code.
    let scale_x = area.width as f64 / ORIG_W;
    let scale_y = area.height as f64 / ORIG_H;
    let to_screen = |x: f64, y: f64| -> (u16, u16) {
        let sx = (x * scale_x) as u16 + area.x;
        let sy = ((ORIG_H - y) * scale_y) as u16 + area.y;
        (
            sx.min(area.x + area.width.saturating_sub(1)),
            sy.min(area.y + area.height.saturating_sub(1)),
        )
    };

    let selected_id = app.selected_node().map(|n| n.id.as_str());

    // Label visibility: top 40 most-connected nodes + always the selected one
    let degree_threshold: i64 = {
        let mut degrees: Vec<i64> = app
            .visible_nodes()
            .iter()
            .map(|(_, n)| n.in_degree + n.out_degree)
            .collect();
        if degrees.len() > 40 {
            degrees.sort_unstable_by(|a, b| b.cmp(a));
            degrees[39]
        } else {
            0
        }
    };

    // Overlay node glyphs — transform through vp() then to_screen(), skip off-display nodes
    for (_idx, node) in app.visible_nodes() {
        if let Some(&(gx, gy)) = app.positions.get(&node.id) {
            let (vx, vy) = vp(gx, gy);
            // skip if outside [0,ORIG_W]×[0,ORIG_H] display space
            if !(0.0..=ORIG_W).contains(&vx) || !(0.0..=ORIG_H).contains(&vy) {
                continue;
            }
            let (sx, sy) = to_screen(vx, vy);
            if sx >= area.x
                && sx < area.x + area.width
                && sy >= area.y
                && sy < area.y + area.height
            {
                let is_selected = selected_id == Some(node.id.as_str());
                let color = GraphWidget::node_color(&node.kind);

                let glyph = if is_selected {
                    "◉"
                } else {
                    GraphWidget::node_glyph(&node.kind)
                };

                let mut style = Style::default().fg(color);
                if is_selected {
                    style = style
                        .add_modifier(Modifier::BOLD)
                        .bg(Color::Rgb(30, 30, 50));
                }
                buf.set_string(sx, sy, glyph, style);

                // Labels for selected + well-connected hubs
                let show_label = is_selected
                    || (node.in_degree + node.out_degree) >= degree_threshold;

                if show_label {
                    let label = truncate_label(&node.name, 16);
                    let label_x = sx.saturating_add(2);
                    let label_y = sy.saturating_sub(1).max(area.y);
                    if label_x + label.len() as u16 <= area.x + area.width {
                        let label_color = if is_selected {
                            color
                        } else {
                            Color::Rgb(130, 130, 155)
                        };
                        buf.set_string(
                            label_x,
                            label_y,
                            &label,
                            Style::default().fg(label_color),
                        );
                    }
                }
            }
        }
    }

    // Zoom indicator — always visible so user knows the feature exists
    {
        let zoom_text = format!(" {:.2}x ", app.zoom);
        let zx = area.x + area.width.saturating_sub(zoom_text.len() as u16 + 1);
        let zy = area.y + 1;
        let (fg, bg) = if (app.zoom - 1.0).abs() > 0.04 {
            (Color::Rgb(220, 220, 255), Color::Rgb(40, 40, 70))
        } else {
            (Color::Rgb(140, 140, 180), Color::Rgb(25, 25, 40))
        };
        buf.set_string(zx, zy, &zoom_text, Style::default().fg(fg).bg(bg));
    }

    draw_legend(buf, area);
}

fn draw_legend(buf: &mut Buffer, area: Rect) {
    const ENTRIES: &[(&str, &str)] = &[
        ("◉", "Function"),
        ("◈", "Class"),
        ("◧", "File"),
        ("⬡", "Module"),
        ("◇", "Type"),
        ("◎", "Author"),
    ];
    let y_start = area.y + area.height.saturating_sub(ENTRIES.len() as u16 + 1);
    for (i, &(glyph, kind)) in ENTRIES.iter().enumerate() {
        let y = y_start + i as u16;
        if y >= area.y + area.height {
            break;
        }
        let color = GraphWidget::node_color(kind);
        buf.set_string(area.x + 1, y, glyph, Style::default().fg(color));
        buf.set_string(
            area.x + 3,
            y,
            kind,
            Style::default().fg(Color::Rgb(85, 85, 105)),
        );
    }
}

fn truncate_label(name: &str, max: usize) -> String {
    if name.chars().count() <= max {
        name.to_string()
    } else {
        name.chars()
            .take(max.saturating_sub(2))
            .chain(['.', '.'])
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use cgx_engine::{Node, Edge};
    use super::super::app::App;
    use std::path::PathBuf;

    fn make_node(id: &str, name: &str, degree: i64) -> Node {
        Node {
            id: id.to_string(),
            kind: "Function".to_string(),
            name: name.to_string(),
            path: "src/test.rs".to_string(),
            line_start: 1,
            line_end: 5,
            language: "rust".to_string(),
            churn: 0.0,
            coupling: 0.0,
            community: 0,
            in_degree: degree,
            out_degree: degree,
        }
    }

    fn make_edge(src: &str, dst: &str) -> Edge {
        Edge {
            id: format!("{}->{}", src, dst),
            src: src.to_string(),
            dst: dst.to_string(),
            kind: "CALLS".to_string(),
            weight: 1.0,
            confidence: 1.0,
        }
    }

    #[test]
    fn test_render_graph_spread() {
        let mut nodes: Vec<Node> = Vec::new();
        let mut edges: Vec<Edge> = Vec::new();

        for i in 0..163 {
            nodes.push(make_node(&format!("fn:{}", i), &format!("func{}", i), 3));
        }

        for i in 0..163 {
            for j in 1..=3 {
                let dst = (i + j * 7) % 163;
                edges.push(make_edge(&format!("fn:{}", i), &format!("fn:{}", dst)));
            }
        }

        let app = App::new(nodes, edges, None, PathBuf::from("/tmp/test"));

        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("failed to create test terminal");
        terminal.draw(|f| {
            let area = f.size();
            // Simulate the layout from render_ui
            let main_chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Horizontal)
                .constraints([
                    ratatui::layout::Constraint::Percentage(60),
                    ratatui::layout::Constraint::Percentage(40),
                ])
                .split(area);
            let graph_area = main_chunks[0];
            let graph_block = ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL);
            let inner_graph = graph_block.inner(graph_area);
            f.render_widget(graph_block, graph_area);
            render_graph(&app, inner_graph, f.buffer_mut());
        })
        .expect("failed to draw graph widget");

        let buf = terminal.backend().buffer().clone();

        // Find all non-empty, non-border cells in the graph area
        let mut min_x = u16::MAX;
        let mut max_x = u16::MIN;
        let mut node_count = 0;

        for y in 1..39 {
            for x in 1..70 {
                let cell = buf.get(x, y);
                // Check for node glyphs (colored symbols)
                if cell.symbol() != " " 
                    && cell.symbol() != "─"
                    && cell.symbol() != "│"
                    && cell.symbol() != "┌"
                    && cell.symbol() != "┐"
                    && cell.symbol() != "└"
                    && cell.symbol() != "┘"
                    && cell.fg != Color::Rgb(60, 60, 80) // border color
                    && cell.fg != Color::Rgb(85, 85, 105) // legend text
                {
                    if x < min_x { min_x = x; }
                    if x > max_x { max_x = x; }
                    node_count += 1;
                }
            }
        }

        println!("Node cells found: {}, x range: [{}, {}]", node_count, min_x, max_x);
        println!("Buffer preview (rows 1-20, cols 1-70):");
        for y in 1..20 {
            let mut line = String::new();
            for x in 1..70 {
                let cell = buf.get(x, y);
                if cell.symbol() == " " {
                    line.push(' ');
                } else {
                    line.push('X');
                }
            }
            println!("{}", line);
        }

        assert!(node_count > 50, "Should find many node cells, got {}", node_count);
        assert!(max_x - min_x > 30, "Nodes should span at least 30 columns, got {}", max_x - min_x);
    }
}
