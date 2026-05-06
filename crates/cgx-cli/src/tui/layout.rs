use std::collections::HashMap;

use fdg_sim::{
    force, Dimensions, ForceGraph, ForceGraphHelper, Simulation, SimulationParameters,
};
use fdg_sim::petgraph::graph::NodeIndex;

use cgx_engine::{Edge, Node};

pub struct GraphSim {
    sim: Simulation<(), ()>,
    #[allow(dead_code)]
    name_to_idx: HashMap<String, NodeIndex>,
}

impl GraphSim {
    pub fn new(
        nodes: &[&Node],
        edges: &[&Edge],
    ) -> Self {
        let mut graph: ForceGraph<(), ()> = ForceGraph::default();
        let mut name_to_idx: HashMap<String, NodeIndex> = HashMap::new();

        for node in nodes {
            let idx = graph.add_force_node(&node.id, ());
            name_to_idx.insert(node.id.clone(), idx);
        }

        for edge in edges {
            if let (Some(&src), Some(&dst)) =
                (name_to_idx.get(&edge.src), name_to_idx.get(&edge.dst))
            {
                if src != dst {
                    graph.add_edge(src, dst, ());
                }
            }
        }

        // Scale ideal distance so nodes can actually fit: ~2× the natural grid spacing.
        // With 163 nodes in a 200×160 area, sqrt(area/N) ≈ 14 → ideal ≈ 28.
        let n = nodes.len().max(1);
        let ideal_dist = (2.0 * (200.0_f64 * 160.0 / n as f64).sqrt()).clamp(15.0, 80.0) as f32;

        let sim_force = force::handy(ideal_dist, 0.92_f32, true, true);
        let params = SimulationParameters::new(0.35, Dimensions::Two, sim_force);
        let mut sim = Simulation::from_graph(graph, params);

        // Use smaller dt (0.01) for numerical stability, more steps to compensate.
        let dt = 0.01_f32;
        let initial_steps = if n < 50 { 400 } else if n < 200 { 600 } else { 900 };
        for _ in 0..initial_steps {
            sim.update(dt);
        }

        Self { sim, name_to_idx }
    }

    pub fn step(&mut self) {
        self.sim.update(0.01);
    }

    pub fn positions(&self) -> HashMap<String, (f64, f64)> {
        let graph = self.sim.get_graph();
        let mut positions: HashMap<String, (f64, f64)> = HashMap::new();
        for node in graph.node_weights() {
            positions.insert(
                node.name.clone(),
                (node.location.x as f64, node.location.y as f64),
            );
        }
        positions
    }

    pub fn _node_count(&self) -> usize {
        self.name_to_idx.len()
    }
}

/// Normalize positions to fill [padding, area-dim - padding] in each axis.
pub fn normalize(
    positions: &mut HashMap<String, (f64, f64)>,
    area_w: f64,
    area_h: f64,
) {
    if positions.is_empty() {
        return;
    }

    let min_x = positions.values().map(|p| p.0).fold(f64::INFINITY, f64::min);
    let max_x = positions
        .values()
        .map(|p| p.0)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = positions.values().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let max_y = positions
        .values()
        .map(|p| p.1)
        .fold(f64::NEG_INFINITY, f64::max);

    let range_x = (max_x - min_x).max(1.0);
    let range_y = (max_y - min_y).max(1.0);
    let padding = 0.08;

    for pos in positions.values_mut() {
        pos.0 = (pos.0 - min_x) / range_x * area_w * (1.0 - 2.0 * padding) + area_w * padding;
        pos.1 = (pos.1 - min_y) / range_y * area_h * (1.0 - 2.0 * padding) + area_h * padding;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cgx_engine::{Edge, Node};

    fn make_node(id: &str, name: &str) -> Node {
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
            in_degree: 0,
            out_degree: 0,
        }
    }

    fn make_edge(src: &str, dst: &str) -> Edge {
        Edge {
            id: format!("{}->{}|CALLS|{}", src, dst, src),
            src: src.to_string(),
            dst: dst.to_string(),
            kind: "CALLS".to_string(),
            weight: 1.0,
            confidence: 1.0,
        }
    }

    #[test]
    fn test_layout_spread() {
        let mut nodes: Vec<Node> = Vec::new();
        let mut edges: Vec<Edge> = Vec::new();

        for i in 0..163 {
            nodes.push(make_node(&format!("fn:{}", i), &format!("func{}", i)));
        }

        for i in 0..163 {
            for j in 1..=3 {
                let dst = (i + j * 7) % 163;
                edges.push(make_edge(&format!("fn:{}", i), &format!("fn:{}", dst)));
            }
        }

        let node_refs: Vec<&Node> = nodes.iter().collect();
        let edge_refs: Vec<&Edge> = edges.iter().collect();

        let sim = GraphSim::new(&node_refs, &edge_refs);
        let mut positions = sim.positions();

        normalize(&mut positions, 200.0, 160.0);

        let min_x = positions
            .values()
            .map(|p| p.0)
            .fold(f64::INFINITY, f64::min);
        let max_x = positions
            .values()
            .map(|p| p.0)
            .fold(f64::NEG_INFINITY, f64::max);
        let min_y = positions
            .values()
            .map(|p| p.1)
            .fold(f64::INFINITY, f64::min);
        let max_y = positions
            .values()
            .map(|p| p.1)
            .fold(f64::NEG_INFINITY, f64::max);

        println!(
            "Layout bounds: x=[{}, {}], y=[{}, {}]",
            min_x, max_x, min_y, max_y
        );

        assert!(
            max_x - min_x > 100.0,
            "x range should be > 100, got {}",
            max_x - min_x
        );
        assert!(
            max_y - min_y > 80.0,
            "y range should be > 80, got {}",
            max_y - min_y
        );
    }

    #[test]
    fn test_step_layout_doesnt_collapse() {
        let mut nodes: Vec<Node> = Vec::new();
        let mut edges: Vec<Edge> = Vec::new();

        for i in 0..163 {
            nodes.push(make_node(&format!("fn:{}", i), &format!("func{}", i)));
        }

        for i in 0..163 {
            for j in 1..=3 {
                let dst = (i + j * 7) % 163;
                edges.push(make_edge(&format!("fn:{}", i), &format!("fn:{}", dst)));
            }
        }

        let node_refs: Vec<&Node> = nodes.iter().collect();
        let edge_refs: Vec<&Edge> = edges.iter().collect();

        let mut sim = GraphSim::new(&node_refs, &edge_refs);

        // Simulate many step iterations
        for _ in 0..100 {
            sim.step();
        }

        let mut positions = sim.positions();
        normalize(&mut positions, 200.0, 160.0);

        let min_x = positions
            .values()
            .map(|p| p.0)
            .fold(f64::INFINITY, f64::min);
        let max_x = positions
            .values()
            .map(|p| p.0)
            .fold(f64::NEG_INFINITY, f64::max);
        let min_y = positions
            .values()
            .map(|p| p.1)
            .fold(f64::INFINITY, f64::min);
        let max_y = positions
            .values()
            .map(|p| p.1)
            .fold(f64::NEG_INFINITY, f64::max);

        println!(
            "After 100 steps: x=[{}, {}], y=[{}, {}]",
            min_x, max_x, min_y, max_y
        );

        assert!(
            max_x - min_x > 50.0,
            "x range should be > 50 after steps, got {}",
            max_x - min_x
        );
        assert!(
            max_y - min_y > 40.0,
            "y range should be > 40 after steps, got {}",
            max_y - min_y
        );
    }
}
