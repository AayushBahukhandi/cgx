use std::collections::HashMap;
use std::path::Path;

use git2::{Repository, Sort};

type OwnerList = Vec<(String, String, f64)>;
type CoChangeList = Vec<(String, String, f64)>;

pub struct GitAnalysis {
    pub file_churn: HashMap<String, f64>,
    pub file_owners: HashMap<String, OwnerList>,
    pub co_changes: CoChangeList,
}

pub fn analyze_repo(repo_path: &Path, file_paths: &[String]) -> anyhow::Result<GitAnalysis> {
    let repo = Repository::open(repo_path)?;

    let (file_churn, co_changes) = compute_churn_and_co_changes(&repo)?;
    let file_owners = compute_blame(&repo, file_paths)?;

    Ok(GitAnalysis {
        file_churn,
        file_owners,
        co_changes,
    })
}

fn compute_churn_and_co_changes(
    repo: &Repository,
) -> anyhow::Result<(HashMap<String, f64>, CoChangeList)> {
    // 90 days for churn scoring; 365 days for co-change pairs (stable repos commit infrequently)
    let churn_cutoff = chrono::Utc::now().timestamp() - 90 * 86400;
    let co_change_cutoff = chrono::Utc::now().timestamp() - 365 * 86400;

    let mut commit_counts: HashMap<String, u32> = HashMap::new();
    let mut pair_counts: HashMap<(String, String), u32> = HashMap::new();
    let mut max_churn: u32 = 0;
    let mut max_co: u32 = 0;

    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(Sort::TIME)?;

    for oid_result in revwalk {
        let oid = match oid_result {
            Ok(o) => o,
            Err(_) => continue,
        };
        let commit = match repo.find_commit(oid) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let commit_ts = commit.time().seconds();
        if commit_ts < co_change_cutoff {
            break;
        }

        let commit_tree = match commit.tree() {
            Ok(t) => t,
            Err(_) => continue,
        };

        let mut parent_tree = None;
        if let Ok(parent) = commit.parent(0) {
            if let Ok(tree) = parent.tree() {
                parent_tree = Some(tree);
            }
        }

        let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&commit_tree), None)?;

        let mut changed_files: Vec<String> = Vec::new();

        diff.foreach(
            &mut |delta, _| {
                if let Some(path) = delta.new_file().path() {
                    if let Some(s) = path.to_str() {
                        changed_files.push(s.to_string());
                    }
                }
                true
            },
            None,
            None,
            None,
        )?;

        changed_files.sort();
        changed_files.dedup();

        // Only count churn for commits within the 90-day window
        if commit_ts >= churn_cutoff {
            for file in &changed_files {
                let count = commit_counts.entry(file.clone()).or_insert(0);
                *count += 1;
                if *count > max_churn {
                    max_churn = *count;
                }
            }
        }

        for i in 0..changed_files.len() {
            for j in (i + 1)..changed_files.len() {
                let pair = (changed_files[i].clone(), changed_files[j].clone());
                let count = pair_counts.entry(pair).or_insert(0);
                *count += 1;
                if *count > max_co {
                    max_co = *count;
                }
            }
        }
    }

    let mut churn_map = HashMap::new();
    if max_churn > 0 {
        for (file, count) in commit_counts {
            churn_map.insert(file, count as f64 / max_churn as f64);
        }
    }

    let mut co_results: CoChangeList = Vec::new();
    let min_co_count = 2u32;
    if max_co > 0 {
        for ((a, b), count) in pair_counts {
            if count >= min_co_count {
                co_results.push((a, b, count as f64 / max_co as f64));
            }
        }
    }

    Ok((churn_map, co_results))
}

fn compute_blame(
    repo: &Repository,
    file_paths: &[String],
) -> anyhow::Result<HashMap<String, OwnerList>> {
    let mut owners: HashMap<String, OwnerList> = HashMap::new();

    for file_path in file_paths {
        let blame = match repo.blame_file(std::path::Path::new(file_path), None) {
            Ok(b) => b,
            Err(e) => {
                tracing::debug!("blame failed for {}: {}", file_path, e);
                continue;
            }
        };

        let mut author_lines: HashMap<String, (String, u32)> = HashMap::new();
        let mut total_lines: u32 = 0;

        for hunk in blame.iter() {
            let sig = hunk.final_signature();
            let name = sig.name().unwrap_or("unknown").to_string();
            let email = sig.email().unwrap_or("unknown").to_string();
            let lines = hunk.lines_in_hunk() as u32;

            let key = email.clone();
            let entry = author_lines.entry(key).or_insert((name, 0));
            entry.1 += lines;
            total_lines += lines;
        }

        if total_lines > 0 {
            let mut file_owners: Vec<(String, String, f64)> = author_lines
                .into_iter()
                .map(|(email, (name, lines))| (name, email, lines as f64 / total_lines as f64))
                .collect();
            file_owners.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
            file_owners.truncate(3);
            owners.insert(file_path.clone(), file_owners);
        }
    }

    Ok(owners)
}
