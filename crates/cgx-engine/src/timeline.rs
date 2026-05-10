use std::path::Path;

use anyhow::Result;
use git2::Repository;

use crate::graph::{GraphDb, SnapshotEntry};

static SOURCE_EXTS: &[&str] = &[
    "ts", "tsx", "js", "jsx", "py", "rs", "go", "java", "cs", "php",
];

pub fn build_timeline(
    repo_path: &Path,
    db: &GraphDb,
    max_commits: usize,
    since: Option<&str>,
) -> Result<Vec<SnapshotEntry>> {
    let repo = Repository::open(repo_path)?;
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(git2::Sort::TIME)?;

    let mut entries = Vec::new();

    for oid in revwalk.take(max_commits) {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;

        let commit_sha = oid.to_string();
        let commit_date = {
            let secs = commit.time().seconds();
            let dt = chrono::DateTime::from_timestamp(secs, 0).unwrap_or_default();
            dt.format("%Y-%m-%d").to_string()
        };

        if let Some(cutoff) = since {
            if commit_date.as_str() < cutoff {
                break;
            }
        }

        let commit_msg = commit.summary().unwrap_or("").to_string();

        // Return cached snapshot if already built
        if let Ok(Some(existing)) = db.get_snapshot_by_sha(&commit_sha) {
            entries.push(existing);
            continue;
        }

        // Count source files in tree
        let tree = commit.tree()?;
        let mut file_count: i64 = 0;
        tree.walk(git2::TreeWalkMode::PreOrder, |_, entry| {
            if let Some(name) = entry.name() {
                if let Some(ext) = name.rsplit('.').next() {
                    if SOURCE_EXTS.contains(&ext) {
                        file_count += 1;
                    }
                }
            }
            git2::TreeWalkResult::Ok
        })?;

        // Diff stats vs first parent
        let (insertions, deletions) = diff_stats(&repo, &commit);

        let snapshot_data = serde_json::json!({
            "file_count": file_count,
            "insertions": insertions,
            "deletions": deletions,
        })
        .to_string();

        let id = format!("snap:{}", &commit_sha[..8]);
        let entry = SnapshotEntry {
            id,
            commit_sha,
            commit_date,
            commit_msg,
            node_count: 0,
            edge_count: 0,
            snapshot_data: Some(snapshot_data),
        };

        let _ = db.upsert_snapshot(&entry);
        entries.push(entry);
    }

    Ok(entries)
}

fn diff_stats(repo: &Repository, commit: &git2::Commit) -> (i64, i64) {
    let tree = match commit.tree() {
        Ok(t) => t,
        Err(_) => return (0, 0),
    };
    let parent_tree = commit
        .parent(0)
        .ok()
        .and_then(|p| p.tree().ok());

    let diff = match repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None) {
        Ok(d) => d,
        Err(_) => return (0, 0),
    };

    let stats = match diff.stats() {
        Ok(s) => s,
        Err(_) => return (0, 0),
    };

    (stats.insertions() as i64, stats.deletions() as i64)
}
