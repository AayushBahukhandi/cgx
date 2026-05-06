use std::path::PathBuf;

use cgx_engine::analyze_repo;

fn fixture_path(relative: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/fixtures");
    path.push(relative);
    path
}

#[test]
fn test_churn_scores() {
    let repo_path = fixture_path("git-sample");
    let file_paths: Vec<String> = vec![
        "src/auth.ts".to_string(),
        "src/db.ts".to_string(),
        "src/router.ts".to_string(),
    ];

    let analysis = analyze_repo(&repo_path, &file_paths).expect("analyze_repo failed");

    let churn = &analysis.file_churn;

    // auth.ts and db.ts changed in 3 of 4 commits -> highest churn (1.0)
    // router.ts changed in 1 of 4 commits -> lower churn
    let auth_churn = churn.get("src/auth.ts").copied().unwrap_or(0.0);
    let db_churn = churn.get("src/db.ts").copied().unwrap_or(0.0);
    let router_churn = churn.get("src/router.ts").copied().unwrap_or(0.0);

    assert!(auth_churn > 0.0, "auth.ts should have churn > 0");
    assert!(db_churn > 0.0, "db.ts should have churn > 0");
    assert!(auth_churn >= router_churn, "auth.ts should have equal or higher churn than router.ts");
    assert!(db_churn >= router_churn, "db.ts should have equal or higher churn than router.ts");
}

#[test]
fn test_co_change_edges() {
    let repo_path = fixture_path("git-sample");
    let file_paths: Vec<String> = vec![
        "src/auth.ts".to_string(),
        "src/db.ts".to_string(),
        "src/router.ts".to_string(),
    ];

    let analysis = analyze_repo(&repo_path, &file_paths).expect("analyze_repo failed");

    let co_changes = &analysis.co_changes;

    // auth.ts and db.ts changed together in commits 1, 2, 4 -> should have a co-change edge
    let has_auth_db = co_changes.iter().any(|(a, b, _)| {
        (a == "src/auth.ts" && b == "src/db.ts") || (a == "src/db.ts" && b == "src/auth.ts")
    });

    assert!(has_auth_db, "should have co-change edge between auth.ts and db.ts");

    // Weight should be > 0.5 (co-changed in most commits)
    if let Some((_, _, weight)) = co_changes.iter().find(|(a, b, _)| {
        (a == "src/auth.ts" && b == "src/db.ts") || (a == "src/db.ts" && b == "src/auth.ts")
    }) {
        assert!(*weight > 0.5, "co-change weight should be > 0.5, got {}", weight);
    }
}

#[test]
fn test_ownership() {
    let repo_path = fixture_path("git-sample");
    let file_paths: Vec<String> = vec![
        "src/auth.ts".to_string(),
        "src/db.ts".to_string(),
        "src/router.ts".to_string(),
    ];

    let analysis = analyze_repo(&repo_path, &file_paths).expect("analyze_repo failed");

    let owners = &analysis.file_owners;

    // auth.ts should have Alice as owner
    let auth_owners = owners.get("src/auth.ts");
    assert!(auth_owners.is_some(), "auth.ts should have ownership data");

    if let Some(auth_owners) = auth_owners {
        let has_alice = auth_owners.iter().any(|(_, email, _)| email == "alice@dev.io");
        assert!(has_alice, "Alice should own auth.ts (she made 3 of 4 commits to this file)");

        // Alice should have > 50% ownership
        if let Some((_, _, pct)) = auth_owners.iter().find(|(_, email, _)| email == "alice@dev.io") {
            assert!(*pct > 0.5, "Alice should own > 50% of auth.ts, got {:.2}", pct);
        }
    }

    // router.ts should have Bob as owner (he added it in commit 3)
    let router_owners = owners.get("src/router.ts");
    if let Some(router_owners) = router_owners {
        let has_bob = router_owners.iter().any(|(_, email, _)| email == "bob@dev.io");
        assert!(has_bob, "Bob should have ownership of router.ts");
    }
}

#[test]
fn test_not_a_git_repo() {
    let non_git_path = fixture_path("ts-sample");
    let file_paths: Vec<String> = vec![];

    let result = analyze_repo(&non_git_path, &file_paths);
    assert!(result.is_err(), "should error for non-git directory");
}
