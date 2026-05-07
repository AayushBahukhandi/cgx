use std::collections::HashMap;
use std::path::PathBuf;

use cgx_engine::{Edge, Node};

use super::layout::{normalize, GraphSim};

#[allow(dead_code)]
pub enum AppMode {
    Normal,
    Search,
    FilterCommunity,
    Help,
    EgoGraph,
}

impl AppMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            AppMode::Normal => "NORMAL",
            AppMode::Search => "SEARCH",
            AppMode::FilterCommunity => "FILTER",
            AppMode::Help => "HELP",
            AppMode::EgoGraph => "EGO",
        }
    }
}

pub struct App {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub selected: Option<usize>,
    pub filter_community: Option<i64>,
    pub search_query: String,
    pub mode: AppMode,
    pub positions: HashMap<String, (f64, f64)>,
    pub help_scroll: usize,
    pub graph_area: (u16, u16),
    pub should_quit: bool,
    pub repo_path: PathBuf,

    // Viewport state: zoom ≥ 0.5, 1.0 = fit-to-screen; pan offsets center in graph space
    pub zoom: f64,
    pub pan_x: f64,
    pub pan_y: f64,

    // Remaining live-layout ticks — stops at 0 so simulation doesn't drift/diverge
    layout_ticks: u32,

    node_index: HashMap<String, usize>,
    edge_index_src: HashMap<String, Vec<usize>>,
    edge_index_dst: HashMap<String, Vec<usize>>,
    visible_node_indices: Vec<usize>,

    sim: Option<GraphSim>,
}

impl App {
    pub fn new(
        nodes: Vec<Node>,
        edges: Vec<Edge>,
        filter_community: Option<i64>,
        repo_path: PathBuf,
    ) -> Self {
        let node_index: HashMap<String, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.clone(), i))
            .collect();

        let mut edge_index_src: HashMap<String, Vec<usize>> = HashMap::new();
        let mut edge_index_dst: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, e) in edges.iter().enumerate() {
            edge_index_src.entry(e.src.clone()).or_default().push(i);
            edge_index_dst.entry(e.dst.clone()).or_default().push(i);
        }

        let visible_node_indices: Vec<usize> = (0..nodes.len())
            .filter(|&i| filter_community.is_none_or(|c| nodes[i].community == c))
            .collect();

        let selected = visible_node_indices.first().copied();

        let visible_nodes: Vec<&Node> = visible_node_indices.iter().map(|&i| &nodes[i]).collect();
        let visible_edges: Vec<&Edge> = edges
            .iter()
            .filter(|e| node_index.contains_key(&e.src) && node_index.contains_key(&e.dst))
            .collect();

        let (sim, positions) = if !visible_nodes.is_empty() {
            let s = GraphSim::new(&visible_nodes, &visible_edges);
            let mut pos = s.positions();
            normalize(&mut pos, 200.0, 160.0);
            (Some(s), pos)
        } else {
            (None, HashMap::new())
        };

        Self {
            nodes,
            edges,
            selected,
            filter_community,
            search_query: String::new(),
            mode: AppMode::Normal,
            positions,
            help_scroll: 0,
            graph_area: (80, 24),
            should_quit: false,
            repo_path,
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            layout_ticks: 120,
            node_index,
            edge_index_src,
            edge_index_dst,
            visible_node_indices,
            sim,
        }
    }

    pub fn selected_node(&self) -> Option<&Node> {
        self.selected.map(|i| &self.nodes[i])
    }

    pub fn visible_nodes(&self) -> Vec<(usize, &Node)> {
        self.visible_node_indices
            .iter()
            .map(|&i| (i, &self.nodes[i]))
            .collect()
    }

    pub fn visible_edges_for_display(&self) -> Vec<&Edge> {
        self.edges
            .iter()
            .filter(|e| {
                self.node_index.contains_key(&e.src)
                    && self.node_index.contains_key(&e.dst)
                    && self.positions.contains_key(&e.src)
                    && self.positions.contains_key(&e.dst)
            })
            .collect()
    }

    pub fn visible_node_count(&self) -> usize {
        self.visible_node_indices.len()
    }

    pub fn callers_of(&self, node_id: &str) -> Vec<&Node> {
        self.edge_index_dst
            .get(node_id)
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|&ei| {
                let edge = &self.edges[ei];
                self.node_index.get(&edge.src).map(|&ni| &self.nodes[ni])
            })
            .collect()
    }

    pub fn callees_of(&self, node_id: &str) -> Vec<&Node> {
        self.edge_index_src
            .get(node_id)
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|&ei| {
                let edge = &self.edges[ei];
                self.node_index.get(&edge.dst).map(|&ni| &self.nodes[ni])
            })
            .collect()
    }

    pub fn select_next(&mut self) {
        if self.visible_node_indices.is_empty() {
            return;
        }
        let current = self
            .selected
            .and_then(|s| self.visible_node_indices.iter().position(|&i| i == s));
        let next = match current {
            Some(pos) => (pos + 1) % self.visible_node_indices.len(),
            None => 0,
        };
        self.selected = Some(self.visible_node_indices[next]);
    }

    pub fn select_prev(&mut self) {
        if self.visible_node_indices.is_empty() {
            return;
        }
        let current = self
            .selected
            .and_then(|s| self.visible_node_indices.iter().position(|&i| i == s));
        let prev = match current {
            Some(pos) => {
                if pos == 0 {
                    self.visible_node_indices.len() - 1
                } else {
                    pos - 1
                }
            }
            None => 0,
        };
        self.selected = Some(self.visible_node_indices[prev]);
    }

    pub fn apply_search_filter(&mut self) {
        if self.search_query.is_empty() {
            self.visible_node_indices = (0..self.nodes.len())
                .filter(|&i| {
                    self.filter_community
                        .is_none_or(|c| self.nodes[i].community == c)
                })
                .collect();
        } else {
            let query = self.search_query.to_lowercase();
            self.visible_node_indices = self
                .nodes
                .iter()
                .enumerate()
                .filter(|(_, n)| {
                    let community_ok = self.filter_community.is_none_or(|c| n.community == c);
                    community_ok
                        && (n.name.to_lowercase().contains(&query)
                            || n.path.to_lowercase().contains(&query)
                            || n.kind.to_lowercase().contains(&query))
                })
                .map(|(i, _)| i)
                .collect();
        }
        if !self.visible_node_indices.is_empty() {
            self.selected = Some(self.visible_node_indices[0]);
        } else {
            self.selected = None;
        }
    }

    pub fn set_community_filter(&mut self, community: Option<i64>) {
        self.filter_community = community;
        self.search_query.clear();
        self.apply_search_filter();
        self.reset_layout();
    }

    pub fn expand_ego(&mut self) {
        if let Some(node_idx) = self.selected {
            let node_id = self.nodes[node_idx].id.clone();
            let neighbor_set: HashMap<String, usize> = self
                .edges
                .iter()
                .filter_map(|e| {
                    if e.src == node_id {
                        self.node_index.get(&e.dst).copied()
                    } else if e.dst == node_id {
                        self.node_index.get(&e.src).copied()
                    } else {
                        None
                    }
                })
                .map(|i| (self.nodes[i].id.clone(), i))
                .collect();

            self.visible_node_indices = vec![node_idx];
            self.visible_node_indices.extend(neighbor_set.into_values());
            self.visible_node_indices.sort();
            self.visible_node_indices.dedup();
            self.reset_layout();
        }
    }

    pub fn zoom_in(&mut self) {
        self.zoom = (self.zoom * 1.25).min(8.0);
    }

    pub fn zoom_out(&mut self) {
        self.zoom = (self.zoom / 1.25).max(0.5);
    }

    pub fn reset_viewport(&mut self) {
        self.zoom = 1.0;
        self.pan_x = 0.0;
        self.pan_y = 0.0;
    }

    // dx/dy in graph-space units; step should be pre-scaled by caller if desired
    pub fn pan(&mut self, dx: f64, dy: f64) {
        self.pan_x = (self.pan_x + dx).clamp(-150.0, 150.0);
        self.pan_y = (self.pan_y + dy).clamp(-120.0, 120.0);
    }

    /// Full reset: clears all filters/ego/search, restores every node, resets layout + viewport.
    pub fn reset_all(&mut self) {
        self.search_query.clear();
        self.filter_community = None;
        self.mode = AppMode::Normal;
        self.visible_node_indices = (0..self.nodes.len()).collect();
        self.selected = self.visible_node_indices.first().copied();
        self.reset_layout();
        self.reset_viewport();
    }

    pub fn reset_layout(&mut self) {
        let visible_nodes: Vec<&Node> = self
            .visible_node_indices
            .iter()
            .map(|&i| &self.nodes[i])
            .collect();
        let visible_edges: Vec<&Edge> = self
            .edges
            .iter()
            .filter(|e| {
                self.node_index.contains_key(&e.src) && self.node_index.contains_key(&e.dst)
            })
            .collect();

        if visible_nodes.is_empty() {
            self.sim = None;
            self.positions.clear();
            return;
        }

        let sim = GraphSim::new(&visible_nodes, &visible_edges);
        let mut pos = sim.positions();
        normalize(&mut pos, 200.0, 160.0);
        self.positions = pos;
        self.sim = Some(sim);
        self.layout_ticks = 120; // allow ~120 live steps after reset then freeze
    }

    pub fn step_layout(&mut self) {
        if self.visible_node_indices.len() > 300 || self.layout_ticks == 0 {
            return;
        }
        self.layout_ticks -= 1;

        if let Some(ref mut sim) = self.sim {
            sim.step();
            let mut pos = sim.positions();

            // If simulation diverged (NaN / inf), freeze and keep last good positions
            let bad = pos.values().any(|&(x, y)| !x.is_finite() || !y.is_finite());
            if bad {
                self.layout_ticks = 0;
                return;
            }

            normalize(&mut pos, 200.0, 160.0);
            self.positions = pos;
        }
    }
}
