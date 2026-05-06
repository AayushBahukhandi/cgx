use std::collections::{HashMap, HashSet};

use crate::graph::{Edge, GraphDb, Node};

pub fn detect_communities(nodes: &[Node], edges: &[Edge]) -> HashMap<String, i64> {
    if nodes.is_empty() {
        return HashMap::new();
    }

    let mut communities: HashMap<String, i64> = HashMap::new();
    let node_ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    let node_idx: HashMap<&str, usize> = node_ids
        .iter()
        .enumerate()
        .map(|(i, id)| (*id, i))
        .collect();
    let n = node_ids.len();

    let mut part: Vec<usize> = (0..n).collect();

    let mut adj: Vec<HashMap<usize, f64>> = vec![HashMap::new(); n];
    let mut k: Vec<f64> = vec![0.0; n];
    let mut total_weight: f64 = 0.0;

    for edge in edges {
        if let (Some(&src_idx), Some(&dst_idx)) = (
            node_idx.get(edge.src.as_str()),
            node_idx.get(edge.dst.as_str()),
        ) {
            if src_idx == dst_idx {
                continue;
            }
            let w = edge.weight.max(0.01) * edge.confidence.max(0.01);
            *adj[src_idx].entry(dst_idx).or_insert(0.0) += w;
            *adj[dst_idx].entry(src_idx).or_insert(0.0) += w;
            k[src_idx] += w;
            k[dst_idx] += w;
            total_weight += w;
        }
    }

    if total_weight == 0.0 {
        for (i, id) in node_ids.iter().enumerate() {
            communities.insert(id.to_string(), i as i64 + 1);
        }
        return communities;
    }

    let m2 = total_weight;

    let mut improved = true;
    let mut iteration = 0;
    let max_iterations = 50;
    while improved && iteration < max_iterations {
        improved = false;
        iteration += 1;

        let mut order: Vec<usize> = (0..n).collect();
        fastrand::shuffle(&mut order);

        for &node in &order {
            let current_comm = part[node];
            let mut best_comm = current_comm;
            let mut best_delta: f64 = 0.0;

            let ki = k[node];
            let ki_in_current = community_edge_weight(node, current_comm, &adj, &part);
            let sigma_tot_current = total_community_weight(current_comm, &part, &k) - ki;

            let mut neighbor_comms: HashSet<usize> = HashSet::new();
            for &neighbor in adj[node].keys() {
                neighbor_comms.insert(part[neighbor]);
            }

            for &neighbor_comm in &neighbor_comms {
                if neighbor_comm == current_comm {
                    continue;
                }

                let ki_in_neighbor = community_edge_weight(node, neighbor_comm, &adj, &part);
                let sigma_tot_neighbor = total_community_weight(neighbor_comm, &part, &k);

                let delta = modularity_delta(
                    ki_in_neighbor,
                    sigma_tot_neighbor,
                    ki,
                    m2,
                ) - modularity_delta(
                    ki_in_current,
                    sigma_tot_current,
                    ki,
                    m2,
                );

                if delta > best_delta {
                    best_delta = delta;
                    best_comm = neighbor_comm;
                }
            }

            if best_comm != current_comm {
                part[node] = best_comm;
                improved = true;
            }
        }
    }

    let mut comm_map: HashMap<usize, i64> = HashMap::new();
    let mut next_comm_id: i64 = 1;
    for &c in &part {
        comm_map.entry(c).or_insert_with(|| {
            let id = next_comm_id;
            next_comm_id += 1;
            id
        });
    }

    for (i, id) in node_ids.iter().enumerate() {
        let comm = comm_map[&part[i]];
        communities.insert(id.to_string(), comm);
    }

    communities
}

fn community_edge_weight(
    node: usize,
    community: usize,
    adj: &[HashMap<usize, f64>],
    part: &[usize],
) -> f64 {
    let mut weight = 0.0;
    for (&neighbor, &w) in &adj[node] {
        if part[neighbor] == community {
            weight += w;
        }
    }
    weight
}

fn total_community_weight(community: usize, part: &[usize], k: &[f64]) -> f64 {
    let mut total = 0.0;
    for (i, &comm) in part.iter().enumerate() {
        if comm == community {
            total += k[i];
        }
    }
    total
}

fn modularity_delta(ki_in: f64, sigma_tot: f64, ki: f64, m2: f64) -> f64 {
    if m2 == 0.0 {
        return 0.0;
    }
    (ki_in / m2) - (sigma_tot * ki / (2.0 * m2 * m2))
}

pub fn run_clustering(db: &GraphDb) -> anyhow::Result<usize> {
    db.clear_communities()?;

    let nodes = db.get_all_nodes()?;
    let edges = db.get_all_edges()?;

    if nodes.is_empty() {
        return Ok(0);
    }

    let community_map = detect_communities(&nodes, &edges);

    let community_count = {
        let mut unique: HashSet<i64> = HashSet::new();
        for &c in community_map.values() {
            unique.insert(c);
        }
        unique.len()
    };

    db.update_node_communities(&community_map)?;

    Ok(community_count)
}
