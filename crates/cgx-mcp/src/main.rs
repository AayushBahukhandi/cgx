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

    eprintln!("cgx-mcp: serving repo at {}", canonical.display());

    server::run(&canonical)?;
    Ok(())
}
