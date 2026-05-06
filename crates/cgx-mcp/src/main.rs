mod server;
mod tools;

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    // Determine repo path from first CLI arg or current directory
    let repo_path: PathBuf = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    let canonical = repo_path.canonicalize().unwrap_or(repo_path);

    eprintln!("cgx-mcp: indexing repo at {}", canonical.display());

    // Verify repo is indexed
    match cgx_engine::GraphDb::open(&canonical) {
        Ok(db) => {
            if db.node_count().unwrap_or(0) == 0 {
                eprintln!("Warning: repo has no indexed graph. Run `cgx analyze` first.");
            }
        }
        Err(_) => {
            eprintln!("Warning: could not open graph database. Run `cgx analyze` first.");
        }
    }

    server::run(&canonical)?;
    Ok(())
}
