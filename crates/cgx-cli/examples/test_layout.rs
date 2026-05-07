use std::path::Path;

use cgx_engine::{GraphDb, Node, Edge};

fn main() {
    let repo_path = Path::new("/Users/aayush/Developer/codegraph");
    let db = match GraphDb::open(repo_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Failed to open graph DB: {e}");
            std::process::exit(1);
        }
    };

    let nodes: Vec<Node> = match db.get_all_nodes() {
        Ok(n) => n,
        Err(e) => {
            eprintln!("Failed to get nodes: {e}");
            std::process::exit(1);
        }
    };
    let edges: Vec<Edge> = match db.get_all_edges() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Failed to get edges: {e}");
            std::process::exit(1);
        }
    };

    println!("Total nodes: {}, edges: {}", nodes.len(), edges.len());

    // Breakdown by kind
    let mut kind_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for n in &nodes {
        *kind_counts.entry(n.kind.clone()).or_default() += 1;
    }
    println!("\nNode kinds:");
    let mut kinds: Vec<_> = kind_counts.iter().collect();
    kinds.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
    for (kind, count) in &kinds {
        println!("  {}: {}", kind, count);
    }

    // Breakdown by edge kind
    let mut edge_kind_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for e in &edges {
        *edge_kind_counts.entry(e.kind.clone()).or_default() += 1;
    }
    println!("\nEdge kinds:");
    let mut ekinds: Vec<_> = edge_kind_counts.iter().collect();
    ekinds.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
    for (kind, count) in &ekinds {
        println!("  {}: {}", kind, count);
    }
}
