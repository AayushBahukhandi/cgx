mod chat;
mod tui;

use clap::{Parser, Subcommand};
use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use cgx_engine::{
    analyze_repo, build_timeline, detect_clones, export_dot, export_graphml, export_json,
    export_mermaid, export_svg, resolve, run_clustering, walk_repo, ClonePair, Edge, EdgeKind,
    GraphDb, Node, NodeKind, ParserRegistry, Registry, RepoEntry, TagRow,
};

use tui::{App, AppMode, GraphWidget};

use anyhow::Context;
use ratatui::layout::Rect;

#[derive(Parser)]
#[command(name = "cgx", version, about = "Codebase Knowledge Graph")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse a repository and print extracted symbols
    Parse {
        path: Option<PathBuf>,

        #[arg(long)]
        json: bool,
    },
    /// Analyze a repository and store the graph
    Analyze {
        path: Option<PathBuf>,

        #[arg(long)]
        force: bool,

        #[arg(long)]
        incremental: bool,

        #[arg(long)]
        watch: bool,

        #[arg(long)]
        quiet: bool,

        #[arg(long)]
        no_git: bool,

        #[arg(long)]
        no_cluster: bool,

        #[arg(long)]
        no_hooks: bool,

        #[arg(long)]
        verbose: bool,
    },
    /// Initialize cgx in the current repository (creates .cgx/config.toml)
    Init {
        /// Project name (defaults to directory name)
        #[arg(long)]
        name: Option<String>,

        /// Non-interactive: create config with defaults
        #[arg(long)]
        yes: bool,
    },
    /// Show status of the current indexed repo
    Status { path: Option<PathBuf> },
    /// List all indexed repos
    List,
    /// Show high-risk files (high churn x coupling)
    Hotspots {
        /// Path to the repository
        #[arg(long)]
        repo: Option<PathBuf>,

        /// Number of results
        #[arg(long, default_value = "10")]
        top: usize,
    },
    /// Show code ownership by contributor
    BlameGraph {
        /// Path to the repository
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    /// Launch interactive terminal graph viewer
    View {
        /// Path to the repository
        #[arg(long)]
        repo: Option<PathBuf>,

        /// Filter to a subfolder
        #[arg(long)]
        filter: Option<String>,

        /// Filter to a community
        #[arg(long)]
        community: Option<i64>,

        /// Launch in browser instead of terminal
        #[arg(long)]
        web: bool,
    },
    /// Start HTTP API server for web UI
    Serve {
        /// Port to listen on
        #[arg(long, default_value = "7373")]
        port: u16,

        /// Path to the repository
        #[arg(long)]
        repo: Option<PathBuf>,

        /// Do not open browser
        #[arg(long)]
        no_open: bool,
    },
    /// Start MCP server (JSON-RPC over stdio)
    Mcp {
        /// Path to the repository
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    /// Configure AI editor integrations
    Setup {
        /// Dry run: show what would be written without writing
        #[arg(long)]
        dry_run: bool,
    },
    /// Print repository summary
    Summary {
        /// Path to the repository
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    /// Query the code graph
    Query {
        #[command(subcommand)]
        cmd: QueryCmd,
    },
    /// Export the graph in various formats
    Export {
        /// Export format
        #[arg(long, default_value = "json")]
        format: String,

        /// Output file path (writes to stdout if omitted)
        #[arg(long)]
        out: Option<PathBuf>,

        /// Path to the repository
        #[arg(long)]
        repo: Option<PathBuf>,

        /// Max nodes for mermaid export (default 80)
        #[arg(long, default_value = "80")]
        max_nodes: usize,
    },
    /// Publish graph to GitHub Pages
    Publish {
        /// Dry run: show what would happen without pushing
        #[arg(long)]
        dry_run: bool,

        /// Print README badge markdown and exit
        #[arg(long)]
        badge: bool,

        /// Path to the repository
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    /// Upload the indexed graph to a GitHub Gist and return a shareable viewer URL
    Share {
        /// GitHub personal access token (or set GITHUB_TOKEN env var)
        #[arg(long)]
        token: Option<String>,

        /// Make the Gist public (default: secret/unlisted)
        #[arg(long)]
        public: bool,

        /// Path to the repository
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    /// Show graph diff between HEAD and a specified commit
    Diff {
        /// Commit or reference to diff against
        commit: String,

        /// Path to the repository
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    /// Show downstream impact of recent changes
    Impact {
        /// Look back N days (default 7). Accepts "7" or "7d".
        #[arg(long, default_value = "7")]
        since: String,

        /// Path to the repository
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    /// List annotation comments (TODO, FIXME, HACK, etc.) across the codebase
    Todos {
        /// Path to the repository
        #[arg(long)]
        repo: Option<PathBuf>,

        /// Filter by annotation kind (TODO, FIXME, HACK, NOTE, BUG, OPTIMIZE, WARN, XXX)
        #[arg(long)]
        kind: Option<String>,

        /// Filter by comment source: code, jsx, or jsx_commented_code
        #[arg(long)]
        comment_type: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show documentation coverage for exported functions
    Docs {
        #[command(subcommand)]
        cmd: DocsCmd,
    },
    /// Show high-complexity functions
    Complexity {
        /// Path to the repository
        #[arg(long)]
        repo: Option<PathBuf>,

        /// Number of results to show
        #[arg(long, default_value = "20")]
        top: usize,

        /// Minimum complexity score (0.0-1.0)
        #[arg(long)]
        threshold: Option<f64>,

        /// Sort by complexity × churn (combined risk score)
        #[arg(long)]
        combined: bool,
    },
    /// Find duplicate or cloned functions
    Dupes {
        /// Path to the repository
        #[arg(long)]
        repo: Option<PathBuf>,

        /// Similarity threshold (0.0-1.0, default 0.80)
        #[arg(long, default_value = "0.80")]
        threshold: f64,

        /// Filter by kind: exact or near
        #[arg(long)]
        kind: Option<String>,
    },
    /// Show test coverage and untested functions
    Test {
        #[command(subcommand)]
        cmd: TestCmd,
    },
    /// Generate architecture explanations for symbols, folders, or the whole repo
    Explain {
        /// Symbol name, file path, or leave empty for folder/repo
        target: Option<String>,

        /// Path to the repository
        #[arg(long)]
        repo: Option<PathBuf>,

        /// Explain a specific community cluster
        #[arg(long)]
        community: Option<i64>,

        /// Generate full onboarding guide for the repo
        #[arg(long)]
        onboard: bool,

        /// Write output to this file instead of stdout
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Check or manage architecture fitness rules
    Rules {
        #[command(subcommand)]
        cmd: RulesCmd,
    },
    /// Generate a structured review brief for a PR or commit range
    Review {
        /// Commit ref, branch, or commit range (e.g. HEAD~5, main, abc..def)
        #[arg(default_value = "HEAD~1")]
        commit: String,

        /// Path to the repository
        #[arg(long)]
        repo: Option<PathBuf>,

        /// Output format: text, markdown, github-actions
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Show dependency health and CVE status
    Deps {
        #[command(subcommand)]
        cmd: DepsCmd,
    },
    /// Show codebase evolution over recent commits
    Timeline {
        /// Path to the repository
        #[arg(long)]
        repo: Option<PathBuf>,

        /// Number of commits to include (default 20)
        #[arg(long, default_value = "20")]
        commits: usize,

        /// Only include commits after this date (YYYY-MM-DD)
        #[arg(long)]
        since: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Run diagnostic checks on your cgx installation
    Doctor {},
    /// Remove indexed data for a repository (or all repositories)
    Clean {
        /// Path to the repository (omit to clean current directory)
        path: Option<PathBuf>,

        /// Clean all indexed repositories
        #[arg(long)]
        all: bool,
    },
    /// Check for updates and show how to upgrade cgx
    Update {
        /// Auto-update the binary (requires cargo or homebrew)
        #[arg(long)]
        auto: bool,
    },
}

#[derive(Subcommand)]
enum QueryCmd {
    /// Find a symbol by name
    Find {
        name: String,
        #[arg(long)]
        kind: Option<String>,
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    /// Get dependencies of a node
    Deps {
        name: String,
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    /// Show all nodes affected if this node changes
    BlastRadius {
        name: String,
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    /// Trace a call path between two symbols
    Chain {
        path: String,
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    /// Show git blame ownership for a file
    Owners {
        path: String,
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    /// Search nodes by name or path
    Search {
        query: String,
        #[arg(long, default_value = "20")]
        limit: u32,
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    /// Show all nodes in a community
    Community {
        id: i64,
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    /// Find unreferenced exports and dead code
    DeadCode {
        #[arg(long)]
        repo: Option<PathBuf>,
        #[arg(long, value_name = "KIND")]
        kind: Option<String>,
        #[arg(long, value_name = "LEVEL")]
        confidence: Option<String>,
        #[arg(long, value_name = "N")]
        community: Option<i64>,
        #[arg(long, value_name = "PREFIX")]
        path: Option<String>,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        summary: bool,
        #[arg(long)]
        safe_to_delete: bool,
    },
}

#[derive(Subcommand)]
enum DocsCmd {
    /// Show documentation coverage for exported functions and classes
    Coverage {
        /// Path to the repository
        #[arg(long)]
        repo: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum RulesCmd {
    /// Run all architecture rules and report violations
    Check {
        /// Path to the repository
        #[arg(long)]
        repo: Option<PathBuf>,
        /// Only run this specific rule by name
        #[arg(long)]
        rule: Option<String>,
        /// Output format: text, github-actions
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// List all defined rules
    List {
        /// Path to the repository
        #[arg(long)]
        repo: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum DepsCmd {
    /// Show full dependency health report with CVE information
    Health {
        /// Path to the repository
        #[arg(long)]
        repo: Option<PathBuf>,
        /// Only show packages with CVEs
        #[arg(long)]
        critical: bool,
    },
    /// Quick CVE count for each dependency
    Audit {
        /// Path to the repository
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    /// Show packages with newer versions available
    Outdated {
        /// Path to the repository
        #[arg(long)]
        repo: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum TestCmd {
    /// Show overall test coverage statistics
    Coverage {
        /// Path to the repository
        #[arg(long)]
        repo: Option<PathBuf>,

        /// Group results by field (e.g. "community")
        #[arg(long)]
        by: Option<String>,
    },
    /// Show untested high-coupling functions ranked by risk
    Gaps {
        /// Path to the repository
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    /// Suggest which tests to write first (prioritized by risk)
    Suggest {
        /// Path to the repository
        #[arg(long)]
        repo: Option<PathBuf>,
    },
}

fn main() -> anyhow::Result<()> {
    // Run before Cli::parse() so the notice is shown even when cgx is invoked
    // with no subcommand (clap calls process::exit after printing help, never
    // returning to the code below parse()).
    let is_update_cmd = std::env::args().nth(1).as_deref() == Some("update");
    if !is_update_cmd {
        maybe_show_update_notice();
    }

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Parse { path, json } => {
            let repo_path = path.unwrap_or_else(|| PathBuf::from("."));
            cmd_parse(&repo_path, json)
        }
        Commands::Analyze {
            path,
            force,
            incremental,
            watch,
            quiet,
            no_git,
            no_cluster,
            no_hooks,
            verbose,
        } => {
            let repo_path = path.unwrap_or_else(|| PathBuf::from("."));
            let resolved_path = resolve_github_path(&repo_path)?;
            if watch {
                cmd_analyze_watch(
                    &resolved_path,
                    force,
                    incremental,
                    quiet,
                    no_git,
                    no_cluster,
                    no_hooks,
                    verbose,
                )
            } else {
                cmd_analyze(
                    &resolved_path,
                    force,
                    incremental,
                    quiet,
                    no_git,
                    no_cluster,
                    no_hooks,
                    verbose,
                )
            }
        }
        Commands::Init { name, yes } => cmd_init(name, yes),
        Commands::Status { path } => {
            let repo_path = path.unwrap_or_else(|| PathBuf::from("."));
            cmd_status(&repo_path)
        }
        Commands::List => cmd_list(),
        Commands::Hotspots { repo, top } => {
            let repo_path = repo.unwrap_or_else(|| PathBuf::from("."));
            cmd_hotspots(&repo_path, top)
        }
        Commands::BlameGraph { repo } => {
            let repo_path = repo.unwrap_or_else(|| PathBuf::from("."));
            cmd_blame_graph(&repo_path)
        }
        Commands::View {
            repo,
            filter,
            community,
            web,
        } => {
            let repo_path = repo.unwrap_or_else(|| PathBuf::from("."));
            if web {
                let rt = tokio::runtime::Runtime::new()?;
                rt.block_on(cmd_view_web(&repo_path, filter.as_deref(), community))
            } else {
                cmd_view(&repo_path, filter.as_deref(), community)
            }
        }
        Commands::Serve {
            port,
            repo,
            no_open,
        } => {
            let repo_path = repo.unwrap_or_else(|| PathBuf::from("."));
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(cmd_serve(&repo_path, port, !no_open))
        }
        Commands::Mcp { repo } => {
            let repo_path = repo.unwrap_or_else(|| PathBuf::from("."));
            let canonical = repo_path.canonicalize().unwrap_or(repo_path);
            cgx_mcp::server::run(&canonical)
        }
        Commands::Setup { dry_run } => cmd_setup(dry_run),
        Commands::Summary { repo } => {
            let repo_path = repo.unwrap_or_else(|| PathBuf::from("."));
            cmd_summary(&repo_path)
        }
        Commands::Query { cmd } => match cmd {
            QueryCmd::Find { name, kind, repo } => cmd_query_find(name, kind, repo),
            QueryCmd::Deps { name, repo } => cmd_query_deps(name, repo),
            QueryCmd::BlastRadius { name, repo } => cmd_query_blast_radius(name, repo),
            QueryCmd::Chain { path, repo } => cmd_query_chain(path, repo),
            QueryCmd::Owners { path, repo } => cmd_query_owners(path, repo),
            QueryCmd::Search { query, limit, repo } => cmd_query_search(query, limit, repo),
            QueryCmd::Community { id, repo } => cmd_query_community(id, repo),
            QueryCmd::DeadCode {
                repo,
                kind,
                confidence,
                community,
                path,
                json,
                summary,
                safe_to_delete,
            } => cmd_query_dead_code(
                repo,
                kind,
                confidence,
                community,
                path,
                json,
                summary,
                safe_to_delete,
            ),
        },
        Commands::Export {
            format,
            out,
            repo,
            max_nodes,
        } => {
            let repo_path = repo.unwrap_or_else(|| PathBuf::from("."));
            cmd_export(&repo_path, &format, out.as_deref(), max_nodes)
        }
        Commands::Publish {
            dry_run,
            badge,
            repo,
        } => {
            let repo_path = repo.unwrap_or_else(|| PathBuf::from("."));
            cmd_publish(&repo_path, dry_run, badge)
        }
        Commands::Share {
            token,
            public,
            repo,
        } => {
            let repo_path = repo.unwrap_or_else(|| PathBuf::from("."));
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(cmd_share(&repo_path, token.as_deref(), public))
        }
        Commands::Diff { commit, repo } => {
            let repo_path = repo.unwrap_or_else(|| PathBuf::from("."));
            cmd_diff(&repo_path, &commit)
        }
        Commands::Impact { since, repo } => {
            let repo_path = repo.unwrap_or_else(|| PathBuf::from("."));
            let since_days = parse_duration_days(&since)?;
            cmd_impact(&repo_path, since_days)
        }
        Commands::Todos {
            repo,
            kind,
            comment_type,
            json,
        } => {
            let repo_path = repo.unwrap_or_else(|| PathBuf::from("."));
            let kind_uc = kind.as_deref().map(|t| t.to_uppercase());
            cmd_todos(
                &repo_path,
                kind_uc.as_deref(),
                comment_type.as_deref(),
                json,
            )
        }
        Commands::Docs { cmd } => match cmd {
            DocsCmd::Coverage { repo } => {
                let repo_path = repo.unwrap_or_else(|| PathBuf::from("."));
                cmd_docs_coverage(&repo_path)
            }
        },
        Commands::Complexity {
            repo,
            top,
            threshold,
            combined,
        } => {
            let repo_path = repo.unwrap_or_else(|| PathBuf::from("."));
            cmd_complexity(&repo_path, top, threshold, combined)
        }
        Commands::Dupes {
            repo,
            threshold,
            kind,
        } => {
            let repo_path = repo.unwrap_or_else(|| PathBuf::from("."));
            cmd_dupes(&repo_path, threshold, kind.as_deref())
        }
        Commands::Explain {
            target,
            repo,
            community,
            onboard,
            out,
        } => {
            let repo_path = repo.unwrap_or_else(|| PathBuf::from("."));
            cmd_explain(
                &repo_path,
                target.as_deref(),
                community,
                onboard,
                out.as_deref(),
            )
        }
        Commands::Rules { cmd } => match cmd {
            RulesCmd::Check { repo, rule, format } => {
                let repo_path = repo.unwrap_or_else(|| PathBuf::from("."));
                cmd_rules_check(&repo_path, rule.as_deref(), &format)
            }
            RulesCmd::List { repo } => {
                let repo_path = repo.unwrap_or_else(|| PathBuf::from("."));
                cmd_rules_list(&repo_path)
            }
        },
        Commands::Review {
            commit,
            repo,
            format,
        } => {
            let repo_path = repo.unwrap_or_else(|| PathBuf::from("."));
            cmd_review(&repo_path, &commit, &format)
        }
        Commands::Deps { cmd } => match cmd {
            DepsCmd::Health { repo, critical } => {
                let repo_path = repo.unwrap_or_else(|| PathBuf::from("."));
                cmd_deps_health(&repo_path, critical)
            }
            DepsCmd::Audit { repo } => {
                let repo_path = repo.unwrap_or_else(|| PathBuf::from("."));
                cmd_deps_audit(&repo_path)
            }
            DepsCmd::Outdated { repo } => {
                let repo_path = repo.unwrap_or_else(|| PathBuf::from("."));
                cmd_deps_outdated(&repo_path)
            }
        },
        Commands::Test { cmd } => match cmd {
            TestCmd::Coverage { repo, by } => {
                let repo_path = repo.unwrap_or_else(|| PathBuf::from("."));
                cmd_test_coverage(&repo_path, by.as_deref())
            }
            TestCmd::Gaps { repo } => {
                let repo_path = repo.unwrap_or_else(|| PathBuf::from("."));
                cmd_test_gaps(&repo_path)
            }
            TestCmd::Suggest { repo } => {
                let repo_path = repo.unwrap_or_else(|| PathBuf::from("."));
                cmd_test_suggest(&repo_path)
            }
        },
        Commands::Timeline {
            repo,
            commits,
            since,
            json,
        } => {
            let repo_path = repo.unwrap_or_else(|| PathBuf::from("."));
            cmd_timeline(&repo_path, commits, since.as_deref(), json)
        }
        Commands::Doctor {} => cmd_doctor(),
        Commands::Clean { path, all } => {
            if all {
                cmd_clean_all()
            } else {
                let repo_path = path.unwrap_or_else(|| PathBuf::from("."));
                cmd_clean(&repo_path)
            }
        }
        Commands::Update { auto } => cmd_update(auto),
    };

    result
}

fn cmd_parse(repo_path: &Path, json: bool) -> anyhow::Result<()> {
    let files = cgx_engine::walk_repo(repo_path)?;
    let registry = cgx_engine::ParserRegistry::new();
    let results = registry.parse_all(&files);

    let mut total_functions = 0usize;
    let mut total_classes = 0usize;
    let mut total_imports = 0usize;

    let mut all_nodes = Vec::new();
    let mut all_edges = Vec::new();

    for result in &results {
        total_functions += result
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Function)
            .count();
        total_classes += result
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Class)
            .count();
        total_imports += result
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Imports)
            .count();

        all_nodes.extend(result.nodes.clone());
        all_edges.extend(result.edges.clone());
    }

    if json {
        let output = serde_json::json!({
            "nodes": &all_nodes,
            "edges": &all_edges,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!(
            "Parsed {} files: {} functions, {} classes, {} imports",
            files.len(),
            total_functions,
            total_classes,
            total_imports
        );
    }

    Ok(())
}

/// If `path` starts with `github:owner/repo`, clone it into `~/.cgx/clones/`
/// and return the cloned directory path. Otherwise return the path unchanged.
fn resolve_github_path(path: &Path) -> anyhow::Result<PathBuf> {
    let path_str = path.to_string_lossy();
    if let Some(spec) = path_str.strip_prefix("github:") {
        let parts: Vec<&str> = spec.split('/').collect();
        if parts.len() != 2 {
            anyhow::bail!(
                "Invalid github: format. Expected: github:owner/repo, got: github:{}",
                spec
            );
        }
        let owner = parts[0];
        let repo = parts[1];
        let clone_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".cgx")
            .join("clones")
            .join(owner)
            .join(repo);

        if clone_dir.exists() {
            println!("  Using existing clone: {}", clone_dir.display());
            // Pull latest
            let status = std::process::Command::new("git")
                .args(["-C", &clone_dir.to_string_lossy(), "pull", "--quiet"])
                .status()?;
            if !status.success() {
                eprintln!("  Warning: git pull failed, using existing files");
            }
        } else {
            let clone_parent = clone_dir.parent().ok_or_else(|| {
                anyhow::anyhow!("clone path has no parent: {}", clone_dir.display())
            })?;
            std::fs::create_dir_all(clone_parent)?;
            let url = format!("https://github.com/{}/{}", owner, repo);
            println!("  Cloning {} into {}...", url, clone_dir.display());
            let status = std::process::Command::new("git")
                .args(["clone", "--depth", "1", &url, &clone_dir.to_string_lossy()])
                .status()?;
            if !status.success() {
                anyhow::bail!("git clone failed for {}", url);
            }
        }
        Ok(clone_dir)
    } else {
        Ok(path.to_path_buf())
    }
}

#[allow(clippy::too_many_arguments)]
fn cmd_analyze(
    repo_path: &Path,
    force: bool,
    incremental: bool,
    quiet: bool,
    no_git: bool,
    no_cluster: bool,
    no_hooks: bool,
    verbose: bool,
) -> anyhow::Result<()> {
    let canonical = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());
    let repo_name = canonical
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let _verbose = verbose;

    // Check if already indexed and handle --force / --incremental
    let db = GraphDb::open(&canonical)?;
    let already_indexed = db.node_count().unwrap_or(0) > 0;

    if incremental {
        if !already_indexed {
            eprintln!("  No existing index found. Running full analyze instead.");
        } else {
            return match cgx_engine::analyze_repo_incremental(
                &canonical, &db, quiet, no_git, no_cluster, verbose,
            ) {
                Ok(true) => {
                    // Update registry entry
                    let mut reg = cgx_engine::Registry::load()?;
                    let node_count = db.node_count()?;
                    let edge_count = db.edge_count()?;
                    let breakdown = db.get_language_breakdown()?;
                    reg.register(cgx_engine::RepoEntry {
                        id: cgx_engine::graph::repo_hash(&canonical),
                        name: repo_name.clone(),
                        path: canonical.clone(),
                        db_path: db.db_path.clone(),
                        indexed_at: chrono::Utc::now().to_rfc3339(),
                        node_count,
                        edge_count,
                        language_breakdown: breakdown,
                    });
                    reg.save()?;

                    // Regenerate skill files
                    let skill_data = cgx_engine::build_skill_data(&db)?;
                    let _ = cgx_engine::write_skill(&canonical, &skill_data);
                    let _ = cgx_engine::write_agents_md(&canonical, &skill_data);

                    if !no_hooks {
                        let _ = cgx_engine::install_git_hooks(&canonical);
                    }

                    if !quiet {
                        println!();
                        println!(
                            "  Graph updated — {} nodes, {} edges",
                            node_count, edge_count
                        );
                        println!();
                        println!("  Generated files:");
                        println!(
                            "    CGX_SKILL.md   \u{2014} skill for any AI assistant (commit this)"
                        );
                        println!("    AGENTS.md      \u{2014} architecture summary (commit this)");
                    }
                    Ok(())
                }
                Ok(false) => Ok(()),
                Err(e) => Err(e),
            };
        }
    }

    if already_indexed && !force {
        if !quiet {
            println!(
                "  Repo already indexed ({} nodes). Use --force to re-index.",
                db.node_count().unwrap_or(0)
            );
        }
        return Ok(());
    }

    // Step 1: Walk files
    let files = walk_repo(&canonical)?;
    let file_count = files.len();

    // Step 2: Parse all files in parallel
    let registry = ParserRegistry::new();
    let results = registry.parse_all(&files);

    let mut all_nodes: Vec<_> = Vec::new();
    let mut all_edges: Vec<_> = Vec::new();
    let mut file_paths: HashSet<String> = HashSet::new();

    for result in &results {
        all_nodes.extend(result.nodes.clone());
        all_edges.extend(result.edges.clone());
    }

    for file in &files {
        file_paths.insert(file.relative_path.clone());
    }

    let parse_nodes_count = all_nodes.len();
    let parse_edges_count = all_edges.len();

    // Step 2.5: Create file nodes
    let mut lang_map: std::collections::HashMap<String, &str> = files
        .iter()
        .map(|f| {
            let lang_str = match f.language {
                cgx_engine::walker::Language::TypeScript => "typescript",
                cgx_engine::walker::Language::JavaScript => "javascript",
                cgx_engine::walker::Language::Python => "python",
                cgx_engine::walker::Language::Rust => "rust",
                cgx_engine::walker::Language::Go => "go",
                cgx_engine::walker::Language::Java => "java",
                cgx_engine::walker::Language::CSharp => "csharp",
                cgx_engine::walker::Language::Php => "php",
                cgx_engine::walker::Language::Unknown => "unknown",
            };
            (f.relative_path.clone(), lang_str)
        })
        .collect();
    let parsed_lang_map = cgx_engine::resolver::build_language_map(&all_nodes);
    for (path, lang) in parsed_lang_map {
        // Never overwrite walker-derived language with "unknown" from parsed map.
        // The walker has the ground truth; parsed_lang_map fills in any gaps.
        if lang != "unknown" {
            lang_map.entry(path).or_insert(lang);
        }
    }
    let file_nodes = cgx_engine::resolver::create_file_nodes(&file_paths, &lang_map);
    all_nodes.extend(file_nodes);

    // Step 3: Resolve cross-file symbols
    let resolved_edges = resolve(&all_nodes, &all_edges, &canonical)?;
    let resolved_count = resolved_edges.len();

    // Step 4: Store in DuckDB
    db.clear()?;

    let db_nodes: Vec<_> = all_nodes
        .iter()
        .map(|n| {
            let lang = lang_map.get(&n.path).copied().unwrap_or("unknown");
            Node::from_def(n, lang)
        })
        .collect();
    let db_edges: Vec<_> = resolved_edges.iter().map(Edge::from_def).collect();

    let _ = db.upsert_nodes(&db_nodes)?;
    let _ = db.upsert_edges(&db_edges)?;

    // Update doc_comment for nodes that have it in metadata
    for result in &results {
        for node_def in &result.nodes {
            if let Some(doc) = node_def
                .metadata
                .get("doc_comment")
                .and_then(|v| v.as_str())
            {
                if !doc.is_empty() {
                    let _ = db.update_node_doc_comment(&node_def.id, doc);
                }
            }
        }
    }

    // Store comment annotation tags (TODO/FIXME/HACK/etc.) extracted from all files
    let tag_rows: Vec<TagRow> = results
        .iter()
        .zip(files.iter())
        .flat_map(|(result, file)| {
            result.comment_tags.iter().map(move |t| TagRow {
                id: format!("tag:{}:{}:{}", file.relative_path, t.line, t.tag_type),
                file_path: file.relative_path.clone(),
                line: t.line,
                tag_type: t.tag_type.clone(),
                text: t.text.clone(),
                comment_type: t.comment_kind.as_str().to_string(),
            })
        })
        .collect();
    db.clear_all_tags()?;
    let tag_count = db.upsert_tags(&tag_rows)?;

    db.update_in_out_degrees()?;

    // Mark test files and update test coverage
    {
        let test_paths: Vec<String> = files
            .iter()
            .filter(|f| cgx_engine::resolver::is_test_path(&f.relative_path))
            .map(|f| f.relative_path.clone())
            .collect();
        let _ = db.mark_test_files(&test_paths);
        let _ = db.update_test_coverage();
    }

    if !quiet {
        println!(
            "  \u{2713} Walking files...            {:>4} files found",
            file_count
        );
        println!(
            "  \u{2713} Parsing (parallel)...       {:>4} nodes, {:>4} edges",
            parse_nodes_count, parse_edges_count
        );
        println!(
            "  \u{2713} Resolving imports...        {:>4} cross-file links resolved",
            resolved_count
        );
        println!(
            "  \u{2713} Storing graph...            saved to {}",
            db.db_path.display()
        );
        if tag_count > 0 {
            println!(
                "  \u{2713} Indexing annotations...     {:>4} TODO/FIXME/HACK tags",
                tag_count
            );
        }
    }

    // Step 5: Git Intelligence
    if !no_git {
        let relative_paths: Vec<String> = files.iter().map(|f| f.relative_path.clone()).collect();
        let valid_paths: std::collections::HashSet<&str> =
            relative_paths.iter().map(|s| s.as_str()).collect();
        match analyze_repo(&canonical, &relative_paths) {
            Ok(analysis) => {
                let mut author_nodes: Vec<Node> = Vec::new();
                let mut co_change_edges: Vec<Edge> = Vec::new();
                let mut owns_edges: Vec<Edge> = Vec::new();
                let mut seen_authors: HashMap<String, String> = HashMap::new();

                for (file_path, churn) in &analysis.file_churn {
                    let file_node_id = format!("file:{}", file_path);
                    let _ = db.upsert_node_scores(&file_node_id, *churn, 0.0);
                }

                for (file_path, owners) in &analysis.file_owners {
                    let file_node_id = format!("file:{}", file_path);
                    for (name, email, pct) in owners {
                        let author_id = format!("author:{}", email);
                        if !seen_authors.contains_key(email) {
                            author_nodes.push(Node {
                                id: author_id.clone(),
                                kind: "Author".to_string(),
                                name: name.clone(),
                                path: String::new(),
                                line_start: 0,
                                line_end: 0,
                                language: String::new(),
                                churn: 0.0,
                                coupling: 0.0,
                                community: 0,
                                in_degree: 0,
                                out_degree: 0,
                                exported: false,
                                is_dead_candidate: false,
                                dead_reason: None,
                                complexity: 0.0,
                                is_test_file: false,
                                test_count: 0,
                                is_tested: false,
                            });
                            seen_authors.insert(email.clone(), name.clone());
                        }
                        owns_edges.push(Edge {
                            id: format!("{}|OWNS|{}", author_id, file_node_id),
                            src: author_id,
                            dst: file_node_id.clone(),
                            kind: "OWNS".to_string(),
                            weight: *pct,
                            confidence: 1.0,
                        });
                    }
                }

                for (file_a, file_b, weight) in &analysis.co_changes {
                    if !valid_paths.contains(file_a.as_str())
                        || !valid_paths.contains(file_b.as_str())
                    {
                        continue;
                    }
                    let id_a = format!("file:{}", file_a);
                    let id_b = format!("file:{}", file_b);
                    // Create bidirectional edges since co-change is symmetric
                    co_change_edges.push(Edge {
                        id: format!("{}|CO_CHANGES|{}", id_a, id_b),
                        src: id_a.clone(),
                        dst: id_b.clone(),
                        kind: "CO_CHANGES".to_string(),
                        weight: *weight,
                        confidence: 1.0,
                    });
                    co_change_edges.push(Edge {
                        id: format!("{}|CO_CHANGES|{}", id_b, id_a),
                        src: id_b,
                        dst: id_a,
                        kind: "CO_CHANGES".to_string(),
                        weight: *weight,
                        confidence: 1.0,
                    });
                }

                let author_count = author_nodes.len();
                let co_count = co_change_edges.len() / 2;
                let owns_count = owns_edges.len();

                let _ = db.upsert_nodes(&author_nodes)?;
                let _ = db.upsert_edges(&owns_edges)?;
                let _ = db.upsert_edges(&co_change_edges)?;

                db.update_in_out_degrees()?;
                db.compute_coupling()?;

                if !quiet {
                    println!(
                        "  \u{2713} Git layer...                {} authors, {} co-change pairs, {} owns edges",
                        author_count, co_count, owns_count
                    );
                }
            }
            Err(_) => {
                if !quiet {
                    println!("  \u{26A0} Git layer...                not a git repo, skipped");
                }
            }
        }
    }

    // Step 6: Clustering (community detection)
    let _community_count = if no_cluster {
        None
    } else {
        match run_clustering(&db) {
            Ok(count) => {
                if count > 0 && !quiet {
                    println!(
                        "  \u{2713} Clustering...              {} communities detected",
                        count
                    );
                }
                Some(count)
            }
            Err(e) => {
                if !quiet {
                    println!("  \u{26A0} Clustering...              failed: {}", e);
                }
                None
            }
        }
    };

    // Step 6.25: Dead code scan
    let dead_report = cgx_engine::detect_dead_code(&db);
    if let Ok(ref report) = dead_report {
        let _ = cgx_engine::mark_dead_candidates(&db, report);
        if !quiet {
            let total = report.total();
            let (high, _medium, _low) = report.count_by_confidence();
            if total > 0 {
                println!(
                    "  \u{25C6} Dead code scan...          {} candidates ({} high confidence)",
                    total, high
                );
                println!("    Run cgx query dead-code to see details");
            }
        }
    }

    // Step 6.5: Store file hashes for incremental indexing
    use sha2::{Digest, Sha256};
    for file in &files {
        let mut hasher = Sha256::new();
        hasher.update(file.content.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        if let Err(e) = db.set_file_hash(&file.relative_path, &hash) {
            eprintln!(
                "  Warning: failed to store hash for {}: {}",
                file.relative_path, e
            );
        }
    }

    // Step 7: Register in registry
    let mut reg = Registry::load()?;
    let lang_breakdown = db.get_language_breakdown()?;
    let node_count = db.node_count()?;
    let edge_count = db.edge_count()?;
    let indexed_at = chrono::Utc::now().to_rfc3339();

    reg.register(RepoEntry {
        id: db.repo_id.clone(),
        name: repo_name.clone(),
        path: canonical.clone(),
        db_path: db.db_path.clone(),
        indexed_at: indexed_at.clone(),
        node_count,
        edge_count,
        language_breakdown: lang_breakdown,
    });
    reg.save()?;

    // Step 8: Generate skill files + install git hooks
    let skill_data = cgx_engine::build_skill_data(&db)?;
    let _ = cgx_engine::write_skill(&canonical, &skill_data);
    let _ = cgx_engine::write_agents_md(&canonical, &skill_data);

    let (hook_pc, hook_pco) = if no_hooks {
        (false, false)
    } else {
        cgx_engine::install_git_hooks(&canonical).unwrap_or((false, false))
    };

    // drop db connection before opening fresh connection
    drop(db);

    if !quiet {
        println!("  \u{2713} Done");
        println!();
        println!(
            "  Graph indexed \u{2014} {} nodes, {} edges",
            node_count, edge_count
        );
        println!();
        println!("  Generated files:");
        println!("    CGX_SKILL.md   \u{2014} skill for any AI assistant (commit this)");
        println!("    AGENTS.md      \u{2014} architecture summary (commit this)");
        println!();
        println!("  AI editor integration:");
        println!("    MCP server:  cgx setup  (Cursor, Claude Code, Windsurf)");
        println!("    Skills:      CGX_SKILL.md is ready \u{2014} works without any setup");
        println!();
        println!("  Explore:");
        println!("    cgx view        terminal graph");
        println!("    cgx view --web  browser graph");
        println!("    cgx hotspots    high-risk files");

        if hook_pc || hook_pco {
            println!();
            println!(
                "  Git hooks:  post-commit{} post-checkout{}",
                if hook_pc { " \u{2713}" } else { "" },
                if hook_pco { " \u{2713}" } else { "" }
            );
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_analyze_watch(
    repo_path: &Path,
    force: bool,
    incremental: bool,
    quiet: bool,
    no_git: bool,
    no_cluster: bool,
    no_hooks: bool,
    verbose: bool,
) -> anyhow::Result<()> {
    use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc::{channel, RecvTimeoutError};
    use std::time::Duration;

    let canonical = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());

    // Run initial analysis
    println!("  cgx analyze --watch  (press Ctrl+C to stop)\n");
    cmd_analyze(
        repo_path,
        force,
        incremental,
        quiet,
        no_git,
        no_cluster,
        no_hooks,
        verbose,
    )?;

    let (tx, rx) = channel::<Result<Event, notify::Error>>();

    let mut watcher: RecommendedWatcher = Watcher::new(
        move |res| {
            let _ = tx.send(res);
        },
        Config::default().with_poll_interval(Duration::from_millis(200)),
    )?;

    watcher.watch(&canonical, RecursiveMode::Recursive)?;

    let debounce_duration = Duration::from_millis(500);
    let mut last_event_time = None;

    println!("  Watching {} for changes...\n", canonical.display());

    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(event)) => {
                // Filter out irrelevant events
                if event.paths.iter().any(|p| {
                    let s = p.to_string_lossy();
                    s.contains("/.git/")
                        || s.contains("/node_modules/")
                        || s.contains("/target/")
                        || s.contains("/dist/")
                        || s.contains("/__pycache__/")
                        || s.contains("/.cgx/")
                        || s.ends_with("CGX_SKILL.md")
                        || s.ends_with("AGENTS.md")
                        || s.ends_with("~")
                        || s.ends_with(".tmp")
                }) {
                    continue;
                }

                if !quiet {
                    for path in &event.paths {
                        println!("  Changed: {}", path.display());
                    }
                }
                last_event_time = Some(std::time::Instant::now());
            }
            Ok(Err(e)) => {
                eprintln!("  Watch error: {}", e);
            }
            Err(RecvTimeoutError::Timeout) => {
                // Check if debounce period has elapsed
                if let Some(last) = last_event_time {
                    if last.elapsed() >= debounce_duration {
                        last_event_time = None;
                        println!("  Re-analyzing...\n");
                        if let Err(e) = cmd_analyze(
                            repo_path, false, // force = false
                            true,  // incremental = true
                            quiet, no_git, no_cluster, no_hooks, verbose,
                        ) {
                            eprintln!("  Analysis error: {}", e);
                        }
                        println!("  Watching for changes...\n");
                    }
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    Ok(())
}

fn cmd_status(repo_path: &Path) -> anyhow::Result<()> {
    let canonical = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());
    let reg = Registry::load()?;

    let entry = reg
        .find_by_path(&canonical)
        .ok_or_else(|| anyhow::anyhow!("No indexed graph found. Run `cgx analyze` first."))?;

    let db = GraphDb::open(&canonical)?;
    let node_count = db.node_count()?;
    let edge_count = db.edge_count()?;

    println!("  Repo:       {}", entry.name);
    println!("  Path:       {}", entry.path.display());
    println!("  Indexed:    {}", entry.indexed_at);
    println!("  Nodes:      {}", node_count);
    println!("  Edges:      {}", edge_count);
    println!("  DB:         {}", entry.db_path.display());

    if !entry.language_breakdown.is_empty() {
        println!("  Languages:");
        let mut langs: Vec<_> = entry.language_breakdown.iter().collect();
        langs.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
        for (lang, pct) in langs {
            println!("    {}  {:.0}%", lang, pct * 100.0);
        }
    }

    Ok(())
}

fn cmd_list() -> anyhow::Result<()> {
    let reg = Registry::load()?;

    if reg.repos.is_empty() {
        println!("No repos indexed. Run `cgx analyze` first.");
        return Ok(());
    }

    println!(
        "{:<36}  {:<8}  {:<8}  {:<20}  {:<40}",
        "ID", "NODES", "EDGES", "INDEXED", "PATH"
    );
    println!("{}", "-".repeat(120));

    for entry in &reg.repos {
        println!(
            "{:<36}  {:<8}  {:<8}  {:<20}  {:<40}",
            entry.id,
            entry.node_count,
            entry.edge_count,
            &entry.indexed_at[..entry.indexed_at.len().min(19)],
            entry.path.display(),
        );
    }

    Ok(())
}

fn cmd_hotspots(repo_path: &Path, top: usize) -> anyhow::Result<()> {
    let canonical = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());
    let db = GraphDb::open(&canonical)?;

    let hotspots = db.get_hotspots(top)?;

    if hotspots.is_empty() {
        println!("No hotspots found. Run `cgx analyze` on a git repo first.");
        return Ok(());
    }

    println!();
    println!("  HOTSPOTS \u{2014} high churn \u{00d7} high coupling");
    println!("  {}", "\u{2500}".repeat(66));
    println!(
        "  {:<3}  {:<28}  {:>6}  {:>8}  {:>7}",
        "#", "File", "Churn", "Coupling", "Callers"
    );

    for (i, (path, churn, coupling, in_degree)) in hotspots.iter().enumerate() {
        println!(
            "  {:<3}  {:<28}  {:>6.2}  {:>8.2}  {:>7}",
            i + 1,
            truncate_path(path, 28),
            churn,
            coupling,
            in_degree
        );
    }

    Ok(())
}

fn cmd_blame_graph(repo_path: &Path) -> anyhow::Result<()> {
    let canonical = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());
    let db = GraphDb::open(&canonical)?;

    let authors = db.get_ownership()?;

    if authors.is_empty() {
        println!("No ownership data found. Run `cgx analyze` on a git repo first.");
        return Ok(());
    }

    let total_files: i64 = authors.iter().map(|(_, c)| c).sum();
    if total_files == 0 {
        return Ok(());
    }

    println!();
    println!("  OWNERSHIP MAP");
    println!("  {}", "\u{2500}".repeat(55));

    for (name, file_count) in &authors {
        let pct = *file_count as f64 / total_files as f64 * 100.0;
        let bar_len = (pct / 100.0 * 20.0) as usize;
        let bar = "\u{2588}".repeat(bar_len) + &"\u{2591}".repeat(20usize.saturating_sub(bar_len));
        println!(
            "  {:<20}  {}  {:.0}%  ({} files)",
            truncate_str(name, 20),
            bar,
            pct,
            file_count
        );
    }

    Ok(())
}

fn truncate_path(path: &str, max: usize) -> String {
    if path.len() <= max {
        path.to_string()
    } else {
        format!("...{}", &path[path.len().saturating_sub(max - 3)..])
    }
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}

fn cmd_export(
    repo_path: &Path,
    format: &str,
    out: Option<&Path>,
    max_nodes: usize,
) -> anyhow::Result<()> {
    let canonical = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());
    let db = GraphDb::open(&canonical)?;

    let node_count = db.node_count()?;
    if node_count == 0 {
        eprintln!("  Warning: repo has no indexed graph. Run `cgx analyze` first.");
    }

    let output = match format.to_lowercase().as_str() {
        "json" => export_json(&db)?,
        "mermaid" => export_mermaid(&db, max_nodes)?,
        "dot" => export_dot(&db)?,
        "svg" => export_svg(&db)?,
        "graphml" => export_graphml(&db)?,
        other => anyhow::bail!(
            "Unknown format: {}. Supported: json, mermaid, dot, svg, graphml",
            other
        ),
    };

    if let Some(out_path) = out {
        std::fs::write(out_path, &output)?;
        eprintln!("  Exported to {}", out_path.display());
    } else {
        println!("{}", output);
    }

    Ok(())
}

// ── TUI / View ──────────────────────────────────────────────────────────

fn cmd_view(repo_path: &Path, filter: Option<&str>, community: Option<i64>) -> anyhow::Result<()> {
    let canonical = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());
    let db = GraphDb::open(&canonical)?;

    let mut nodes = db.get_all_nodes()?;
    if nodes.is_empty() {
        anyhow::bail!(
            "No indexed graph found at {}. Run `cgx analyze` first.",
            canonical.display()
        );
    }

    if let Some(f) = filter {
        let f_norm = if f.ends_with('/') {
            f.to_string()
        } else {
            format!("{}/", f)
        };
        nodes.retain(|n| n.path.starts_with(f) || n.path.starts_with(&f_norm));
        if nodes.is_empty() {
            anyhow::bail!("No nodes match filter: {}", f);
        }
    }

    let node_ids: HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    let mut edges = db.get_all_edges()?;
    edges.retain(|e| node_ids.contains(e.src.as_str()) && node_ids.contains(e.dst.as_str()));

    let mut app = App::new(nodes, edges, community, canonical.clone());

    run_tui(&mut app)?;
    Ok(())
}

fn run_tui(app: &mut App) -> anyhow::Result<()> {
    use crossterm::{
        event::EnableMouseCapture,
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    };
    use ratatui::backend::CrosstermBackend;
    use ratatui::Terminal;

    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context(
        "Failed to initialize terminal. Make sure you are running in an interactive terminal.",
    )?;

    enable_raw_mode()
        .context("Failed to enable raw mode. cgx view requires an interactive terminal.")?;
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)
        .context("Failed to enter alternate screen.")?;

    let result = run_event_loop(app, &mut terminal);

    use crossterm::event::DisableMouseCapture;
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);

    result
}

// Returns the node kind string if the click lands on a legend entry.
fn legend_kind_at_click(
    col: u16,
    row: u16,
    _term_width: u16,
    term_height: u16,
) -> Option<&'static str> {
    const ENTRIES: &[&str] = &["Function", "Class", "File", "Module", "Type", "Author"];
    // Legend is drawn in inner_graph (x=1, y=1, h=term_height-2).
    // draw_legend places y_start = inner_y + inner_h - (len+1) = 1 + (term_height-2) - 7 = term_height-8
    let inner_h = term_height.saturating_sub(2);
    let y_start = 1u16 + inner_h.saturating_sub(ENTRIES.len() as u16 + 1);
    // glyph at col 2, kind text at col 4..4+kind.len()
    if !(2..=14).contains(&col) {
        return None;
    }
    for (i, &kind) in ENTRIES.iter().enumerate() {
        if row == y_start + i as u16 {
            return Some(kind);
        }
    }
    None
}

fn find_node_at_click(
    app: &App,
    col: u16,
    row: u16,
    term_width: u16,
    term_height: u16,
) -> Option<usize> {
    // Must match coordinate transforms in graph_widget.rs exactly.
    const ORIG_W: f64 = 200.0;
    const ORIG_H: f64 = 160.0;

    // Ratatui Percentage(60) uses integer math: width * 60 / 100
    let graph_block_w = term_width * 60 / 100;
    let inner_x = 1u16;
    let inner_y = 1u16;
    let inner_w = graph_block_w.saturating_sub(2);
    let inner_h = term_height.saturating_sub(2);

    if col < inner_x || row < inner_y {
        return None;
    }
    if col >= inner_x + inner_w || row >= inner_y + inner_h {
        return None;
    }

    // Viewport transform — must exactly match vp() in graph_widget.rs
    let zoom = app.zoom;
    let pan_x = app.pan_x;
    let pan_y = app.pan_y;
    let vp = |gx: f64, gy: f64| -> (f64, f64) {
        (
            (gx - ORIG_W / 2.0 - pan_x) * zoom + ORIG_W / 2.0,
            (gy - ORIG_H / 2.0 - pan_y) * zoom + ORIG_H / 2.0,
        )
    };

    let scale_x = inner_w as f64 / ORIG_W;
    let scale_y = inner_h as f64 / ORIG_H;
    let to_screen = |vx: f64, vy: f64| -> (u16, u16) {
        let sx = (vx * scale_x) as u16 + inner_x;
        let sy = ((ORIG_H - vy) * scale_y) as u16 + inner_y;
        (
            sx.min(inner_x + inner_w.saturating_sub(1)),
            sy.min(inner_y + inner_h.saturating_sub(1)),
        )
    };

    let mut best: Option<(i32, usize)> = None;

    for (node_idx, node) in app.visible_nodes() {
        if let Some(&(gx, gy)) = app.positions.get(&node.id) {
            let (vx, vy) = vp(gx, gy);
            if !(0.0..=ORIG_W).contains(&vx) || !(0.0..=ORIG_H).contains(&vy) {
                continue;
            }
            let (sx, sy) = to_screen(vx, vy);

            // Dot hit: 2-cell radius around the node glyph
            let dot_hit =
                (col as i32 - sx as i32).abs() <= 1 && (row as i32 - sy as i32).abs() <= 1;

            // Label hit: label drawn at (sx+2, sy-1) — start must match render exactly
            let label_len = node.name.chars().count().min(16) as u16;
            let label_row = sy.saturating_sub(1);
            let label_hit = row == label_row
                && col >= sx.saturating_add(2)
                && col < sx.saturating_add(2).saturating_add(label_len);

            if dot_hit || label_hit {
                let dist_sq = (col as i32 - sx as i32).pow(2) + (row as i32 - sy as i32).pow(2);
                if best.is_none_or(|(d, _)| dist_sq < d) {
                    best = Some((dist_sq, node_idx));
                }
            }
        }
    }

    best.map(|(_, i)| i)
}

fn run_event_loop<B: ratatui::backend::Backend>(
    app: &mut App,
    terminal: &mut ratatui::Terminal<B>,
) -> anyhow::Result<()> {
    use crossterm::event::{self, Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};

    let tick_rate = Duration::from_millis(50);
    let mut layout_tick: u64 = 0;

    loop {
        app.graph_area = {
            let size = terminal.size()?;
            let w = (size.width.saturating_sub(1) as f64 * 0.6) as u16;
            (w, size.height.saturating_sub(2))
        };

        terminal.draw(|f| render_ui(f, app))?;

        if app.should_quit {
            break;
        }

        if event::poll(tick_rate)? {
            let event = event::read()?;
            match app.mode {
                AppMode::Normal => match event {
                    Event::Key(key) if key.kind != KeyEventKind::Release => {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => {
                                app.should_quit = true;
                            }
                            KeyCode::Char('/') => {
                                app.mode = AppMode::Search;
                                app.search_query.clear();
                            }
                            KeyCode::Char('f') => {
                                app.mode = AppMode::FilterCommunity;
                                app.search_query.clear();
                            }
                            KeyCode::Char('e') => {
                                app.expand_ego();
                            }
                            KeyCode::Char('r') => {
                                app.reset_all();
                            }
                            KeyCode::Char('?') => {
                                app.mode = AppMode::Help;
                                app.help_scroll = 0;
                            }
                            KeyCode::Tab => {
                                app.select_next();
                            }
                            KeyCode::BackTab => {
                                app.select_prev();
                            }
                            KeyCode::Enter => {}
                            KeyCode::Down | KeyCode::Char('j') => {
                                app.select_next();
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                app.select_prev();
                            }
                            // Zoom
                            KeyCode::Char('+') | KeyCode::Char('=') => {
                                app.zoom_in();
                            }
                            KeyCode::Char('-') => {
                                app.zoom_out();
                            }
                            KeyCode::Char('0') => {
                                app.reset_viewport();
                            }
                            // Pan (WASD — graph-space units, scaled by zoom so feel is consistent)
                            KeyCode::Char('w') => {
                                let step = 15.0 / app.zoom;
                                app.pan(0.0, step);
                            }
                            KeyCode::Char('s') => {
                                let step = 15.0 / app.zoom;
                                app.pan(0.0, -step);
                            }
                            KeyCode::Char('a') => {
                                let step = 15.0 / app.zoom;
                                app.pan(-step, 0.0);
                            }
                            KeyCode::Char('d') => {
                                let step = 15.0 / app.zoom;
                                app.pan(step, 0.0);
                            }
                            _ => {}
                        }
                    }
                    Event::Mouse(me) => {
                        match me.kind {
                            MouseEventKind::Down(MouseButton::Left) => {
                                let size = terminal.size()?;
                                // Check legend click first (bottom-left of graph panel)
                                if let Some(kind) =
                                    legend_kind_at_click(me.column, me.row, size.width, size.height)
                                {
                                    app.search_query = kind.to_string();
                                    app.apply_search_filter();
                                    app.reset_layout();
                                } else if let Some(idx) = find_node_at_click(
                                    app,
                                    me.column,
                                    me.row,
                                    size.width,
                                    size.height,
                                ) {
                                    app.selected = Some(idx);
                                }
                            }
                            MouseEventKind::ScrollUp => {
                                app.zoom_in();
                            }
                            MouseEventKind::ScrollDown => {
                                app.zoom_out();
                            }
                            _ => {}
                        }
                    }
                    Event::Resize(_, _) => {
                        app.reset_layout(); // keep filters/viewport on resize
                    }
                    _ => {}
                },
                AppMode::Search => match event {
                    Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                        KeyCode::Esc => {
                            app.mode = AppMode::Normal;
                            app.search_query.clear();
                            app.apply_search_filter();
                        }
                        KeyCode::Enter => {
                            app.apply_search_filter();
                            app.mode = AppMode::Normal;
                            app.reset_layout();
                        }
                        KeyCode::Char(c) => {
                            app.search_query.push(c);
                        }
                        KeyCode::Backspace => {
                            app.search_query.pop();
                        }
                        _ => {}
                    },
                    _ => {}
                },
                AppMode::FilterCommunity => match event {
                    Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                        KeyCode::Esc => {
                            app.mode = AppMode::Normal;
                            app.search_query.clear();
                            app.set_community_filter(None);
                        }
                        KeyCode::Enter => {
                            let input = app.search_query.trim().to_string();
                            app.mode = AppMode::Normal;
                            if input.is_empty() {
                                app.set_community_filter(None);
                            } else if let Ok(c) = input.parse::<i64>() {
                                app.set_community_filter(Some(c));
                            }
                        }
                        KeyCode::Char(c) => {
                            app.search_query.push(c);
                        }
                        KeyCode::Backspace => {
                            app.search_query.pop();
                        }
                        _ => {}
                    },
                    _ => {}
                },
                AppMode::Help => match event {
                    Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                        KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => {
                            app.mode = AppMode::Normal;
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            app.help_scroll = app.help_scroll.saturating_add(1);
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            app.help_scroll = app.help_scroll.saturating_sub(1);
                        }
                        _ => {}
                    },
                    _ => {}
                },
                AppMode::EgoGraph => match event {
                    Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                        KeyCode::Char('r') => {
                            app.reset_all();
                        }
                        KeyCode::Char('q') | KeyCode::Esc => {
                            app.should_quit = true;
                        }
                        _ => {
                            app.mode = AppMode::Normal;
                        }
                    },
                    _ => {
                        app.mode = AppMode::Normal;
                    }
                },
            }
        }

        layout_tick = (layout_tick + 1) % 3;
        if layout_tick == 0 {
            app.step_layout();
        }
    }

    Ok(())
}

fn render_ui(f: &mut ratatui::Frame, app: &mut App) {
    use ratatui::{
        layout::{Constraint, Direction, Layout, Rect},
        style::{Color, Style},
        widgets::{Block, Borders},
    };

    let size = f.size();

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(size);

    let graph_area = main_chunks[0];
    let inspector_area = main_chunks[1];

    let status_area = Rect {
        x: 0,
        y: size.height.saturating_sub(2),
        width: size.width,
        height: 2,
    };

    // Graph panel
    let graph_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(60, 60, 80)))
        .style(Style::default().bg(Color::Rgb(10, 10, 15)));
    let inner_graph = graph_block.inner(graph_area);
    f.render_widget(graph_block, graph_area);
    tui::graph_widget::render_graph(app, inner_graph, f.buffer_mut());

    // Inspector panel
    render_inspector(f, inspector_area, app);

    // Status bar
    render_status_bar(f, status_area, app);

    // Modal overlays
    match app.mode {
        AppMode::Search => render_search_overlay(f, size, app),
        AppMode::FilterCommunity => render_filter_community_overlay(f, size, app),
        AppMode::Help => render_help_overlay(f, size, app),
        _ => {}
    }
}

fn render_inspector(f: &mut ratatui::Frame, area: Rect, app: &App) {
    use ratatui::{
        style::{Color, Modifier, Style},
        text::{Line, Span, Text},
        widgets::{Block, Borders, Paragraph, Wrap},
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(60, 60, 80)))
        .style(Style::default().bg(Color::Rgb(17, 17, 24)));
    f.render_widget(block.clone(), area);
    let inner = block.inner(area);

    let selected = app.selected_node();
    let mut lines: Vec<Line> = Vec::new();

    if let Some(node) = selected {
        let kind_color = match node.kind.as_str() {
            "Function" => Color::Rgb(0, 255, 136),
            "Class" => Color::Rgb(59, 130, 246),
            "File" => Color::Rgb(245, 158, 11),
            "Module" => Color::Rgb(139, 92, 246),
            "Variable" => Color::Rgb(52, 211, 153),
            "Type" => Color::Rgb(168, 85, 247),
            "Author" => Color::Rgb(236, 72, 153),
            _ => Color::Gray,
        };

        lines.push(Line::from(vec![
            Span::styled(
                format!(" {} ", node.kind),
                Style::default().fg(Color::Black).bg(kind_color),
            ),
            Span::raw(" "),
            Span::styled(
                &node.name,
                Style::default().fg(kind_color).add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::raw(""));

        if !node.path.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("File:  ", Style::default().fg(Color::Rgb(100, 100, 120))),
                Span::raw(&node.path),
            ]));
        }

        if node.line_start > 0 {
            lines.push(Line::from(vec![
                Span::styled("Lines: ", Style::default().fg(Color::Rgb(100, 100, 120))),
                Span::raw(format!("{}-{}", node.line_start, node.line_end)),
            ]));
        }

        lines.push(Line::raw(""));

        // Churn bar
        let churn_pct = (node.churn * 100.0).min(100.0) as usize;
        let bar_filled = churn_pct * 20 / 100;
        let bar_empty = 20 - bar_filled;
        lines.push(Line::from(vec![
            Span::styled("Churn: ", Style::default().fg(Color::Rgb(100, 100, 120))),
            Span::styled(
                "\u{2588}".repeat(bar_filled),
                Style::default().fg(Color::Rgb(239, 68, 68)),
            ),
            Span::styled(
                "\u{2591}".repeat(bar_empty),
                Style::default().fg(Color::Rgb(60, 60, 70)),
            ),
            Span::raw(format!(" {:.2}", node.churn)),
        ]));

        // Coupling bar
        let coup_pct = (node.coupling * 100.0).min(100.0) as usize;
        let bar_filled = coup_pct * 20 / 100;
        let bar_empty = 20 - bar_filled;
        lines.push(Line::from(vec![
            Span::styled("Coup:  ", Style::default().fg(Color::Rgb(100, 100, 120))),
            Span::styled(
                "\u{2588}".repeat(bar_filled),
                Style::default().fg(Color::Rgb(59, 130, 246)),
            ),
            Span::styled(
                "\u{2591}".repeat(bar_empty),
                Style::default().fg(Color::Rgb(60, 60, 70)),
            ),
            Span::raw(format!(" {:.2}", node.coupling)),
        ]));

        if node.community > 0 {
            lines.push(Line::from(vec![
                Span::styled("Comm:  ", Style::default().fg(Color::Rgb(100, 100, 120))),
                Span::styled(
                    format!("#{}", node.community),
                    Style::default()
                        .fg(Color::Rgb(139, 92, 246))
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        }

        lines.push(Line::raw(""));
        lines.push(Line::styled(
            format!("  in:{}  out:{}", node.in_degree, node.out_degree),
            Style::default().fg(Color::Rgb(100, 100, 120)),
        ));

        // Callers
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            "\u{2500}\u{2500}\u{2500} Callers \u{2500}\u{2500}\u{2500}",
            Style::default().fg(Color::Rgb(80, 80, 100)),
        ));
        let callers = app.callers_of(&node.id);
        if callers.is_empty() {
            lines.push(Line::styled(
                "  (none)",
                Style::default().fg(Color::Rgb(80, 80, 90)),
            ));
        } else {
            for caller in callers.iter().take(8) {
                let c = GraphWidget::node_color(&caller.kind);
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(&caller.name, Style::default().fg(c)),
                    Span::styled(
                        format!("  ({})", caller.kind),
                        Style::default().fg(Color::Rgb(80, 80, 90)),
                    ),
                ]));
            }
        }

        // Callees
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            "\u{2500}\u{2500}\u{2500} Callees \u{2500}\u{2500}\u{2500}",
            Style::default().fg(Color::Rgb(80, 80, 100)),
        ));
        let callees = app.callees_of(&node.id);
        if callees.is_empty() {
            lines.push(Line::styled(
                "  (none)",
                Style::default().fg(Color::Rgb(80, 80, 90)),
            ));
        } else {
            for callee in callees.iter().take(8) {
                let c = GraphWidget::node_color(&callee.kind);
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(&callee.name, Style::default().fg(c)),
                    Span::styled(
                        format!("  ({})", callee.kind),
                        Style::default().fg(Color::Rgb(80, 80, 90)),
                    ),
                ]));
            }
        }

        // Code snippet
        if node.line_start > 0 && !node.path.is_empty() {
            let file_path = app.repo_path.join(&node.path);
            if let Ok(content) = std::fs::read_to_string(&file_path) {
                let all_lines: Vec<&str> = content.lines().collect();
                let from = (node.line_start as usize).saturating_sub(1);
                let to = (node.line_end as usize).min(all_lines.len());
                let snippet_lines = &all_lines[from..to.min(from + 20)];

                lines.push(Line::raw(""));
                lines.push(Line::styled(
                    "\u{2500}\u{2500}\u{2500} Snippet \u{2500}\u{2500}\u{2500}",
                    Style::default().fg(Color::Rgb(80, 80, 100)),
                ));
                for (i, code_line) in snippet_lines.iter().enumerate() {
                    let lineno = node.line_start as usize + i;
                    let trimmed = if code_line.len() > 52 {
                        format!("{}…", &code_line[..52])
                    } else {
                        code_line.to_string()
                    };
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("{:>4} ", lineno),
                            Style::default().fg(Color::Rgb(60, 60, 80)),
                        ),
                        Span::styled(trimmed, Style::default().fg(Color::Rgb(180, 180, 200))),
                    ]));
                }
            }
        }
    } else {
        lines.push(Line::styled(
            "No node selected",
            Style::default().fg(Color::Rgb(100, 100, 120)),
        ));
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            "Use Tab / jk / arrows to navigate",
            Style::default().fg(Color::Rgb(80, 80, 90)),
        ));
    }

    let paragraph = Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false });
    f.render_widget(paragraph, inner);
}

fn render_status_bar(f: &mut ratatui::Frame, area: Rect, app: &App) {
    use ratatui::{
        style::{Color, Modifier, Style},
        text::{Line, Span, Text},
        widgets::{Block, Borders, Paragraph},
    };

    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::Rgb(60, 60, 80)))
        .style(Style::default().bg(Color::Rgb(17, 17, 24)));
    f.render_widget(block.clone(), area);
    let inner = block.inner(area);

    let status_text = format!(
        "{} visible · {} edges",
        app.visible_node_count(),
        app.visible_edges_for_display().len()
    );

    let mut spans = vec![Span::styled(
        status_text,
        Style::default().fg(Color::Rgb(100, 100, 120)),
    )];

    if let Some(c) = app.filter_community {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            format!("#{}", c),
            Style::default()
                .fg(Color::Rgb(139, 92, 246))
                .add_modifier(Modifier::BOLD),
        ));
    }

    if !app.search_query.is_empty() {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            format!("\"{}\"", app.search_query),
            Style::default().fg(Color::Rgb(59, 130, 246)),
        ));
    }

    let mode_color = match &app.mode {
        AppMode::Normal => Color::Rgb(80, 80, 90),
        AppMode::Search => Color::Rgb(59, 130, 246),
        AppMode::FilterCommunity => Color::Rgb(139, 92, 246),
        AppMode::Help => Color::Rgb(245, 158, 11),
        AppMode::EgoGraph => Color::Rgb(0, 255, 136),
    };
    spans.push(Span::raw("  "));
    spans.push(Span::styled(
        format!("[{}]", app.mode.as_str()),
        Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
    ));

    spans.push(Span::raw("  "));
    spans.push(Span::styled(
        format!("{:.2}x", app.zoom),
        Style::default().fg(if (app.zoom - 1.0).abs() > 0.04 {
            Color::Rgb(200, 200, 255)
        } else {
            Color::Rgb(80, 80, 110)
        }),
    ));

    spans.push(Span::styled(
        "  [q]uit [/]search [f]ilter [e]go [r]eset [+/-/scroll]zoom [wasd]pan [?]help",
        Style::default().fg(Color::Rgb(60, 60, 70)),
    ));

    let paragraph = Paragraph::new(Text::from(Line::from(spans)));
    f.render_widget(paragraph, inner);
}

fn render_search_overlay(f: &mut ratatui::Frame, size: Rect, app: &App) {
    use ratatui::{
        style::{Color, Modifier, Style},
        text::{Line, Span, Text},
        widgets::{Block, Borders, Clear, Paragraph},
    };

    let popup_area = centered_rect(50, 5, size);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(59, 130, 246)))
        .style(Style::default().bg(Color::Rgb(17, 17, 24)));
    f.render_widget(block.clone(), popup_area);
    let inner = block.inner(popup_area);

    let text = Text::from(vec![
        Line::styled(
            "Search Nodes",
            Style::default()
                .fg(Color::Rgb(59, 130, 246))
                .add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::from(vec![
            Span::raw("> "),
            Span::styled(
                &app.search_query,
                Style::default().fg(Color::Rgb(255, 255, 255)),
            ),
        ]),
        Line::raw(""),
        Line::styled(
            "Enter: apply   Esc: cancel",
            Style::default().fg(Color::Rgb(80, 80, 90)),
        ),
    ]);

    let paragraph = Paragraph::new(text);
    f.render_widget(paragraph, inner);
}

fn render_filter_community_overlay(f: &mut ratatui::Frame, size: Rect, app: &App) {
    use ratatui::{
        style::{Color, Modifier, Style},
        text::{Line, Span, Text},
        widgets::{Block, Borders, Clear, Paragraph},
    };

    let popup_area = centered_rect(50, 5, size);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(139, 92, 246)))
        .style(Style::default().bg(Color::Rgb(17, 17, 24)));
    f.render_widget(block.clone(), popup_area);
    let inner = block.inner(popup_area);

    let text = Text::from(vec![
        Line::styled(
            "Filter by Community",
            Style::default()
                .fg(Color::Rgb(139, 92, 246))
                .add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::from(vec![
            Span::raw("Community #: "),
            Span::styled(
                &app.search_query,
                Style::default().fg(Color::Rgb(255, 255, 255)),
            ),
        ]),
        Line::raw(""),
        Line::styled(
            "Enter: apply   Esc: clear filter",
            Style::default().fg(Color::Rgb(80, 80, 90)),
        ),
    ]);

    let paragraph = Paragraph::new(text);
    f.render_widget(paragraph, inner);
}

fn render_help_overlay(f: &mut ratatui::Frame, size: Rect, _app: &App) {
    use ratatui::{
        style::{Color, Modifier, Style},
        text::{Line, Text},
        widgets::{Block, Borders, Clear, Paragraph},
    };

    let popup_area = centered_rect(56, 22, size);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(100, 100, 120)))
        .style(Style::default().bg(Color::Rgb(17, 17, 24)));
    f.render_widget(block.clone(), popup_area);
    let inner = block.inner(popup_area);

    let help_lines = vec![
        Line::styled(
            "  Navigation",
            Style::default()
                .fg(Color::Rgb(200, 200, 255))
                .add_modifier(Modifier::BOLD),
        ),
        Line::raw("  Tab / j/↓     Next node"),
        Line::raw("  S-Tab / k/↑   Previous node"),
        Line::raw("  /             Search nodes"),
        Line::raw("  f             Filter by community"),
        Line::raw("  e             Ego-graph (selected + neighbors)"),
        Line::raw("  r             Reset layout + viewport"),
        Line::raw("  q / Esc       Quit"),
        Line::raw(""),
        Line::styled(
            "  Zoom & Pan",
            Style::default()
                .fg(Color::Rgb(200, 200, 255))
                .add_modifier(Modifier::BOLD),
        ),
        Line::raw("  + / =         Zoom in"),
        Line::raw("  -             Zoom out"),
        Line::raw("  0             Reset zoom & pan"),
        Line::raw("  w/a/s/d       Pan up / left / down / right"),
        Line::raw("  Scroll        Zoom in / out"),
        Line::raw("  Click         Select node or its label"),
        Line::raw(""),
        Line::raw(""),
        Line::styled(
            "  Resources",
            Style::default()
                .fg(Color::Rgb(200, 200, 255))
                .add_modifier(Modifier::BOLD),
        ),
        Line::raw("  Docs & issues: github.com/AayushBahukhandi/cgx"),
        Line::raw("  Annotation tags: cgx todos"),
        Line::raw(""),
        Line::styled(
            "  Esc / ?  close this help",
            Style::default().fg(Color::Rgb(80, 80, 90)),
        ),
    ];

    let paragraph = Paragraph::new(Text::from(help_lines));
    f.render_widget(paragraph, inner);
}

fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let popup_width = r.width * percent_x / 100;
    let popup_height = height.min(r.height);

    let x = r.x + (r.width.saturating_sub(popup_width)) / 2;
    let y = r.y + (r.height.saturating_sub(popup_height)) / 2;

    Rect {
        x,
        y,
        width: popup_width,
        height: popup_height,
    }
}

// ── HTTP Server / Web UI ────────────────────────────────────────────────

// Web UI is embedded into the binary at compile time from packages/web-ui/dist/.
// In debug builds rust-embed serves from the filesystem (fast iteration).
// In release builds everything is bundled — no external files needed.
#[derive(rust_embed::RustEmbed)]
#[folder = "web-ui-dist"]
struct WebUiAssets;

async fn serve_ui_asset(uri: axum::http::Uri) -> axum::response::Response {
    use axum::response::IntoResponse;
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match WebUiAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                [(axum::http::header::CONTENT_TYPE, mime.to_string())],
                content.data.into_owned(),
            )
                .into_response()
        }
        None => {
            // SPA fallback: unknown paths get index.html so client-side routing works
            match WebUiAssets::get("index.html") {
                Some(content) => (
                    [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
                    content.data.into_owned(),
                )
                    .into_response(),
                None => axum::http::StatusCode::NOT_FOUND.into_response(),
            }
        }
    }
}

async fn cmd_serve(repo_path: &Path, port: u16, open_browser: bool) -> anyhow::Result<()> {
    use axum::{routing::get, routing::post, Router};
    use tower_http::cors::CorsLayer;

    let canonical = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());

    let graph_path = canonical.clone();

    let handle_graph = move || {
        let p = graph_path.clone();
        async move { api_graph(p).await }
    };

    let snippet_repo = canonical.clone();
    let open_repo = canonical.clone();

    let handle_snippet = move |query: axum::extract::Query<SnippetQuery>| {
        let p = snippet_repo.clone();
        async move { api_snippet(p, query).await }
    };

    let handle_open = move |query: axum::extract::Query<OpenQuery>| {
        let p = open_repo.clone();
        async move { api_open(p, query).await }
    };

    let chat_repo = canonical.clone();
    let handle_chat = move |body: axum::extract::Json<chat::ChatRequest>| {
        let p = chat_repo.clone();
        async move { chat::chat_stream(p, body.0).await }
    };

    let app = Router::new()
        .route("/api/graph", get(handle_graph))
        .route("/api/repos", get(api_repos))
        .route("/api/repos/{id}/graph", get(api_repo_graph))
        .route("/api/snippet", get(handle_snippet))
        .route("/api/open", get(handle_open))
        .route("/api/chat", post(handle_chat))
        .layer(CorsLayer::permissive())
        .fallback(serve_ui_asset);

    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .context(format!("Failed to bind to {}", addr))?;

    let url = format!("http://localhost:{}", port);
    eprintln!("  cgx serve  — listening on {}", url);

    if open_browser {
        let _ = open::that(&url);
    }

    eprintln!("  Press Ctrl+C to stop");

    axum::serve(listener, app).await?;

    Ok(())
}

async fn api_graph(repo_path: PathBuf) -> axum::response::Response {
    use axum::response::IntoResponse;
    match build_graph_json(&repo_path) {
        Ok(json) => axum::Json(json).into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Error: {}", e),
        )
            .into_response(),
    }
}

async fn api_repos() -> axum::response::Response {
    use axum::response::IntoResponse;
    match Registry::load() {
        Ok(reg) => {
            let repos: Vec<serde_json::Value> = reg
                .repos
                .iter()
                .map(|entry| {
                    serde_json::json!({
                        "id": entry.id,
                        "name": entry.name,
                        "path": entry.path.to_string_lossy(),
                        "node_count": entry.node_count,
                        "edge_count": entry.edge_count,
                        "indexed_at": entry.indexed_at,
                        "language_breakdown": entry.language_breakdown,
                    })
                })
                .collect();
            axum::Json(repos).into_response()
        }
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Error: {}", e),
        )
            .into_response(),
    }
}

async fn api_repo_graph(
    axum::extract::Path(id): axum::extract::Path<String>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    match Registry::load() {
        Ok(reg) => match reg.find_by_id(&id) {
            Some(entry) => match build_graph_json(&entry.path) {
                Ok(json) => axum::Json(json).into_response(),
                Err(e) => (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Error: {}", e),
                )
                    .into_response(),
            },
            None => (
                axum::http::StatusCode::NOT_FOUND,
                format!("Repo not found: {}", id),
            )
                .into_response(),
        },
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Error: {}", e),
        )
            .into_response(),
    }
}

fn build_graph_json(repo_path: &Path) -> anyhow::Result<serde_json::Value> {
    let canonical = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());
    let db = GraphDb::open(repo_path)?;
    let nodes = db.get_all_nodes()?;
    let edges = db.get_all_edges()?;
    let communities = db.get_communities()?;
    let lang_breakdown = db.get_language_breakdown()?;
    let node_count = db.node_count()?;
    let edge_count = db.edge_count()?;
    let repo_name = canonical
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let communities_json: Vec<serde_json::Value> = communities
        .iter()
        .map(|(id, label, count, top_nodes)| {
            serde_json::json!({
                "id": id,
                "label": label,
                "node_count": count,
                "top_nodes": top_nodes,
            })
        })
        .collect();

    let nodes_json: Vec<serde_json::Value> = nodes
        .iter()
        .map(|n| {
            serde_json::json!({
                "id": n.id,
                "kind": n.kind,
                "name": n.name,
                "path": n.path,
                "line_start": n.line_start,
                "line_end": n.line_end,
                "language": n.language,
                "churn": n.churn,
                "coupling": n.coupling,
                "community": n.community,
                "in_degree": n.in_degree,
                "out_degree": n.out_degree,
            })
        })
        .collect();

    let edges_json: Vec<serde_json::Value> = edges
        .iter()
        .map(|e| {
            serde_json::json!({
                "id": e.id,
                "src": e.src,
                "dst": e.dst,
                "kind": e.kind,
                "weight": e.weight,
                "confidence": e.confidence,
            })
        })
        .collect();

    Ok(serde_json::json!({
        "meta": {
            "repo_id": db.repo_id,
            "repo_name": repo_name,
            "node_count": node_count,
            "edge_count": edge_count,
            "language_breakdown": lang_breakdown,
            "community_count": communities.len(),
        },
        "nodes": nodes_json,
        "edges": edges_json,
        "communities": communities_json,
    }))
}

// ── Snippet + Editor Open ────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct SnippetQuery {
    path: String,
    from: Option<u32>,
    to: Option<u32>,
}

#[derive(serde::Deserialize)]
struct OpenQuery {
    path: String,
    line: Option<u32>,
}

fn validate_repo_path(repo_root: &Path, user_path: &str) -> Option<PathBuf> {
    let candidate = repo_root.join(user_path);
    let canonical = candidate.canonicalize().ok()?;
    let root_canonical = repo_root.canonicalize().ok()?;
    if canonical.starts_with(&root_canonical) {
        Some(canonical)
    } else {
        None
    }
}

fn contains_parent_dir(path: &str) -> bool {
    Path::new(path)
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
}

async fn api_snippet(
    repo_path: PathBuf,
    axum::extract::Query(query): axum::extract::Query<SnippetQuery>,
) -> axum::response::Response {
    use axum::response::IntoResponse;

    // Block path traversal BEFORE any filesystem access
    if contains_parent_dir(&query.path) {
        return (axum::http::StatusCode::FORBIDDEN, "Path traversal blocked").into_response();
    }

    let candidate = repo_path.join(&query.path);

    if !candidate.exists() {
        return (axum::http::StatusCode::NOT_FOUND, "File not found").into_response();
    }

    let resolved = match validate_repo_path(&repo_path, &query.path) {
        Some(p) => p,
        None => {
            return (axum::http::StatusCode::FORBIDDEN, "Path traversal blocked").into_response();
        }
    };

    if !resolved.is_file() {
        return (axum::http::StatusCode::NOT_FOUND, "File not found").into_response();
    }

    let content = match std::fs::read_to_string(&resolved) {
        Ok(c) => c,
        Err(_) => {
            return (axum::http::StatusCode::NOT_FOUND, "Cannot read file").into_response();
        }
    };

    let lines: Vec<&str> = content.lines().collect();

    let from = query.from.unwrap_or(1).max(1) as usize;
    let to = query.to.unwrap_or(lines.len() as u32).max(from as u32) as usize;
    let to = to.min(lines.len());

    if from > lines.len() {
        return axum::Json(serde_json::json!({
            "path": query.path,
            "from": from,
            "to": to,
            "lines": [],
            "language": detect_snippet_language(&query.path),
            "total_lines": lines.len(),
        }))
        .into_response();
    }

    let snippet: Vec<serde_json::Value> = lines[from - 1..to]
        .iter()
        .enumerate()
        .map(|(i, line)| {
            serde_json::json!({
                "num": from + i,
                "text": line,
            })
        })
        .collect();

    axum::Json(serde_json::json!({
        "path": query.path,
        "from": from,
        "to": to,
        "lines": snippet,
        "language": detect_snippet_language(&query.path),
        "total_lines": lines.len(),
    }))
    .into_response()
}

fn detect_snippet_language(path: &str) -> &str {
    let lower = path.to_lowercase();
    if lower.ends_with(".ts") || lower.ends_with(".tsx") {
        "typescript"
    } else if lower.ends_with(".js") || lower.ends_with(".jsx") || lower.ends_with(".mjs") {
        "javascript"
    } else if lower.ends_with(".py") {
        "python"
    } else if lower.ends_with(".rs") {
        "rust"
    } else if lower.ends_with(".go") {
        "go"
    } else if lower.ends_with(".java") {
        "java"
    } else if lower.ends_with(".cs") {
        "csharp"
    } else if lower.ends_with(".json") {
        "json"
    } else if lower.ends_with(".md") {
        "markdown"
    } else if lower.ends_with(".html") {
        "html"
    } else if lower.ends_with(".css") {
        "css"
    } else {
        "text"
    }
}

async fn api_open(
    repo_path: PathBuf,
    axum::extract::Query(query): axum::extract::Query<OpenQuery>,
) -> axum::response::Response {
    use axum::response::IntoResponse;

    // Block path traversal BEFORE any filesystem access
    if contains_parent_dir(&query.path) {
        return (axum::http::StatusCode::FORBIDDEN, "Path traversal blocked").into_response();
    }

    let resolved = match validate_repo_path(&repo_path, &query.path) {
        Some(p) => p,
        None => {
            return (axum::http::StatusCode::FORBIDDEN, "Path traversal blocked").into_response();
        }
    };

    let line = query.line.unwrap_or(1).max(1);
    let path_str = resolved.to_string_lossy().to_string();
    let goto = format!("{}:{}", path_str, line);

    let success = try_open_editor("code", &goto)
        || try_open_editor("cursor", &goto)
        || try_open_editor("nvim", &goto);

    if success {
        axum::Json(serde_json::json!({
            "opened": true,
            "path": path_str,
            "line": line,
            "editor": "auto-detected",
        }))
        .into_response()
    } else {
        (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            format!("No editor found. Path: {}", goto),
        )
            .into_response()
    }
}

fn try_open_editor(editor: &str, goto: &str) -> bool {
    let args: &[&str] = match editor {
        "code" | "cursor" => &["--goto", goto],
        "nvim" => &[goto],
        _ => &[],
    };
    std::process::Command::new(editor)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .is_ok()
}

async fn cmd_view_web(
    repo_path: &Path,
    _filter: Option<&str>,
    _community: Option<i64>,
) -> anyhow::Result<()> {
    let canonical = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());

    // Auto-analyze if not indexed
    let needs_index = match GraphDb::open(&canonical) {
        Ok(db) => db.node_count().unwrap_or(0) == 0,
        Err(_) => true,
    };
    if needs_index {
        eprintln!("  No indexed graph found — running analysis first...");
        cmd_analyze(&canonical, false, false, false, false, false, false, false)?;
    }

    let port = 7373u16;
    let url = format!("http://localhost:{}", port);

    // Check if the server is already running on this port
    match tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await {
        Ok(listener) => {
            // Port is free, drop the test listener and start the real server
            drop(listener);
        }
        Err(_) => {
            // Port is in use — server already running
            eprintln!("  Server already running on {}", url);
            let _ = open::that(&url);
            return Ok(());
        }
    }

    // Start server in background
    let serve_path = canonical.clone();

    eprintln!("  Starting cgx serve...");
    eprintln!("  Opening {} ...", url);

    tokio::spawn(async move {
        if let Err(e) = cmd_serve(&serve_path, port, false).await {
            eprintln!("  Server error: {}", e);
        }
    });

    // Give the server a moment to start
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let _ = open::that(&url);

    // Keep running until Ctrl+C
    eprintln!("  Press Ctrl+C to stop");
    tokio::signal::ctrl_c().await?;

    Ok(())
}

// ── Query / Summary / Setup ─────────────────────────────────────────────

fn cmd_summary(repo_path: &Path) -> anyhow::Result<()> {
    let canonical = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());
    let db = GraphDb::open(&canonical)?;
    let data = cgx_engine::build_skill_data(&db)?;

    println!();
    println!("  REPOSITORY SUMMARY");
    println!("  {}", "\u{2500}".repeat(50));
    println!(
        "  Nodes:     {} ({} functions, {} classes, {} files)",
        data.node_count, data.function_count, data.class_count, data.file_count
    );
    println!("  Edges:     {}", data.edge_count);
    println!("  Languages: {}", data.language_breakdown);
    println!("  Indexed:   {}", data.indexed_at);

    if !data.top_communities.is_empty() {
        println!();
        println!("  TOP COMMUNITIES");
        for c in &data.top_communities {
            println!("    #{} — {} ({} nodes)", c.id, c.label, c.node_count);
        }
    }

    if !data.hotspots.is_empty() {
        println!();
        println!("  HOTSPOTS (high churn × coupling)");
        for n in &data.hotspots {
            println!(
                "    {} — churn {:.2}, {} callers",
                n.path, n.churn, n.in_degree
            );
        }
    }

    if !data.entry_points.is_empty() {
        println!();
        println!("  ENTRY POINTS");
        for n in &data.entry_points {
            println!("    {} ({})", n.name, n.kind);
        }
    }

    if !data.god_nodes.is_empty() {
        println!();
        println!("  GOD NODES (most depended-on)");
        for n in &data.god_nodes {
            println!("    {} — {} callers", n.name, n.in_degree);
        }
    }

    Ok(())
}

fn resolve_repo(repo: Option<PathBuf>) -> PathBuf {
    let p = repo.unwrap_or_else(|| PathBuf::from("."));
    p.canonicalize().unwrap_or(p)
}

fn resolve_id(all_nodes: &[cgx_engine::Node], name_or_id: &str) -> Option<String> {
    if all_nodes.iter().any(|n| n.id == name_or_id) {
        return Some(name_or_id.to_string());
    }
    let query = name_or_id.to_lowercase();
    all_nodes
        .iter()
        .find(|n| n.name.to_lowercase() == query)
        .map(|n| n.id.clone())
        .or_else(|| {
            all_nodes
                .iter()
                .find(|n| n.name.to_lowercase().contains(&query))
                .map(|n| n.id.clone())
        })
}

const CLAUDE_SKILL_MD: &str = include_str!("claude_skill.md");

fn install_claude_skill(home: &str, cgx_path: &str, dry_run: bool) {
    let skill_dir = format!("{}/.claude/skills/cgx", home);
    let skill_file = format!("{}/SKILL.md", skill_dir);
    let claude_md = format!("{}/.claude/CLAUDE.md", home);

    if dry_run {
        println!("  → Claude Code skill — would write {}", skill_file);
        return;
    }

    // Write SKILL.md
    if std::fs::create_dir_all(&skill_dir).is_ok() {
        let content = CLAUDE_SKILL_MD.replace("{{CGX_PATH}}", cgx_path);
        if std::fs::write(&skill_file, content).is_ok() {
            println!("  ✓ Claude Code skill  — {}", skill_file);
        }
    }

    // Patch ~/.claude/CLAUDE.md to register /cgx
    let entry = "\n# cgx\n- **cgx** (`~/.claude/skills/cgx/SKILL.md`) - index any Git repo as a queryable knowledge graph. Trigger: `/cgx`\nWhen the user types `/cgx`, invoke the Skill tool with `skill: \"cgx\"` before doing anything else.\n".to_string();
    if Path::new(&claude_md).exists() {
        if let Ok(existing) = std::fs::read_to_string(&claude_md) {
            if !existing.contains("skills/cgx/SKILL.md") {
                let updated = format!("{}{}", existing, entry);
                let _ = std::fs::write(&claude_md, updated);
                println!("  ✓ Registered /cgx in {}", claude_md);
            }
        }
    } else {
        let _ = std::fs::write(&claude_md, entry.trim_start());
        println!("  ✓ Created {} with /cgx registration", claude_md);
    }
}

fn cmd_setup(dry_run: bool) -> anyhow::Result<()> {
    let home = std::env::var("HOME").unwrap_or_default();
    let cgx_path = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "cgx".to_string());

    let editors: Vec<(&str, String, &str, &str)> = vec![
        (
            "Claude Code",
            format!("{}/.claude/settings.json", home),
            "mcpServers",
            "mcpServers",
        ),
        (
            "Cursor",
            format!("{}/.cursor/mcp.json", home),
            "mcpServers",
            "mcpServers",
        ),
        (
            "VS Code",
            format!("{}/.vscode/settings.json", home),
            "mcp.servers",
            "mcp.servers",
        ),
        (
            "Windsurf",
            format!("{}/.windsurf/mcp.json", home),
            "mcpServers",
            "mcpServers",
        ),
        (
            "Zed",
            format!("{}/.config/zed/settings.json", home),
            "context_servers",
            "context_servers",
        ),
    ];

    println!("  cgx setup \u{2014} configuring AI editor integrations\n");

    for (name, config_path, merge_key, _display_key) in &editors {
        let exists = Path::new(&config_path).exists();

        if dry_run {
            if exists {
                println!(
                    "  \u{2713} {} \u{2014} {} (would update)",
                    name, config_path
                );
            } else {
                println!("  \u{2717} {} \u{2014} not detected", name);
            }
            continue;
        }

        if !exists {
            println!("  \u{2717} {} \u{2014} not detected", name);
            continue;
        }

        // Try to read and merge existing config
        if let Ok(content) = std::fs::read_to_string(config_path) {
            if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&content) {
                // Navigate to the correct nested key (e.g., mcp.servers -> {"cgx": ...})
                let parts: Vec<&str> = merge_key.split('.').collect();
                if parts.len() == 2 {
                    let inner = json
                        .as_object_mut()
                        .and_then(|m| {
                            m.entry(parts[0])
                                .or_insert_with(|| serde_json::json!({}))
                                .as_object_mut()
                        })
                        .map(|m| m.entry(parts[1]).or_insert_with(|| serde_json::json!({})));
                    if let Some(target) = inner {
                        target["cgx"] = serde_json::json!({
                            "command": &cgx_path,
                            "args": ["mcp"],
                            "env": {}
                        });
                    }
                } else if let Some(obj) = json.as_object_mut() {
                    obj.entry(parts[0]).or_insert_with(|| serde_json::json!({}))["cgx"] = serde_json::json!({
                        "command": &cgx_path,
                        "args": ["mcp"],
                        "env": {}
                    });
                }
                if let Ok(pretty) = serde_json::to_string_pretty(&json) {
                    let _ = std::fs::write(config_path, pretty);
                    println!("  \u{2713} {} \u{2014} updated {}", name, config_path);
                    continue;
                }
            }
        }
        println!(
            "  \u{26A0} {} \u{2014} could not parse {} (not valid JSON)",
            name, config_path
        );
    }

    // Install Claude Code skill (~/.claude/skills/cgx/SKILL.md)
    install_claude_skill(&home, &cgx_path, dry_run);

    println!();
    println!("  Restart your editor for changes to take effect.");

    // Print manual config instructions for all editors
    println!();
    println!("  Manual MCP configuration (if auto-detect missed your editor):");
    println!("  {}", "\u{2500}".repeat(60));
    println!();
    println!("  Cursor, Claude Code, Windsurf, Codex — mcp.json:");
    println!("    {{");
    println!("      \"mcpServers\": {{");
    println!("        \"cgx\": {{");
    println!("          \"command\": \"{}\",", cgx_path);
    println!("          \"args\": [\"mcp\"],");
    println!("          \"env\": {{}}");
    println!("        }}");
    println!("      }}");
    println!("    }}");
    println!();
    println!("  VS Code — settings.json:");
    println!("    {{");
    println!("      \"mcp.servers\": {{");
    println!("        \"cgx\": {{");
    println!("          \"command\": \"{}\",", cgx_path);
    println!("          \"args\": [\"mcp\"],");
    println!("          \"env\": {{}}");
    println!("        }}");
    println!("      }}");
    println!("    }}");
    println!();
    println!("  Zed — settings.json:");
    println!("    {{");
    println!("      \"context_servers\": {{");
    println!("        \"cgx\": {{");
    println!("          \"command\": \"{}\",", cgx_path);
    println!("          \"args\": [\"mcp\"],");
    println!("          \"env\": {{}}");
    println!("        }}");
    println!("      }}");
    println!("    }}");
    println!();
    Ok(())
}

fn cmd_init(name: Option<String>, yes: bool) -> anyhow::Result<()> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let config_path = cwd.join(".cgx").join("config.toml");

    if config_path.exists() {
        println!();
        println!("  cgx init");
        println!("  {}", "\u{2500}".repeat(60));
        println!();
        println!("  .cgx/config.toml already exists.");
        println!();
        println!("  To regenerate, delete it first:");
        println!("    rm .cgx/config.toml");
        println!();
        return Ok(());
    }

    let default_name = cwd
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "cgx-project".to_string());

    let project_name = name.unwrap_or_else(|| {
        if yes {
            default_name.clone()
        } else {
            println!();
            println!("  cgx init — guided first-run setup");
            println!("  {}", "\u{2500}".repeat(60));
            println!();
            print!("  Project name [{}]: ", default_name);
            use std::io::Write;
            let _ = std::io::stdout().flush();
            let mut input = String::new();
            let _ = std::io::stdin().read_line(&mut input);
            let trimmed = input.trim();
            if trimmed.is_empty() {
                default_name.clone()
            } else {
                trimmed.to_string()
            }
        }
    });

    let mut config = cgx_engine::CgxConfig::default();
    config.project.name = project_name.clone();

    if !yes {
        println!();
        println!("  Default chat provider [ollama]:");
        print!("  Options: openai, anthropic, ollama, openai-compatible\n  > ");
        use std::io::Write;
        let _ = std::io::stdout().flush();
        let mut input = String::new();
        let _ = std::io::stdin().read_line(&mut input);
        let trimmed = input.trim();
        if !trimmed.is_empty() {
            config.chat.provider = trimmed.to_string();
        }

        println!();
        println!("  Default chat model [codellama]:");
        print!("  > ");
        let _ = std::io::stdout().flush();
        let mut input = String::new();
        let _ = std::io::stdin().read_line(&mut input);
        let trimmed = input.trim();
        if !trimmed.is_empty() {
            config.chat.model = trimmed.to_string();
        }

        println!();
        println!("  HTTP server port [7373]:");
        print!("  > ");
        let _ = std::io::stdout().flush();
        let mut input = String::new();
        let _ = std::io::stdin().read_line(&mut input);
        let trimmed = input.trim();
        if let Ok(port) = trimmed.parse::<u16>() {
            config.serve.port = port;
        }
    }

    config.save(&cwd)?;

    println!();
    println!("  \u{2713} Created .cgx/config.toml");
    println!();
    println!("  Project: {}", project_name);
    println!("  Provider: {}", config.chat.provider);
    println!("  Model: {}", config.chat.model);
    println!("  Port: {}", config.serve.port);
    println!();
    println!("  Next steps:");
    println!("    cgx analyze              # index your codebase");
    println!("    cgx setup                # configure AI editor integrations");
    println!("    cgx view --web           # explore the graph");
    println!();

    Ok(())
}

fn cmd_query_find(name: String, kind: Option<String>, repo: Option<PathBuf>) -> anyhow::Result<()> {
    let db = GraphDb::open(&resolve_repo(repo))?;
    let all = db.get_all_nodes()?;
    let query = name.to_lowercase();
    let mut results: Vec<_> = all
        .iter()
        .filter(|n| {
            if let Some(ref k) = kind {
                if n.kind != *k {
                    return false;
                }
            }
            n.name.to_lowercase().contains(&query) || n.id.to_lowercase().contains(&query)
        })
        .collect();
    // Rank: exact name match first, then by in_degree descending (most depended-on)
    results.sort_by(|a, b| {
        let a_exact = a.name.to_lowercase() == query;
        let b_exact = b.name.to_lowercase() == query;
        b_exact
            .cmp(&a_exact)
            .then_with(|| b.in_degree.cmp(&a.in_degree))
    });
    for n in results.iter().take(20) {
        println!("  {}  {:<12}  {}:{}", n.kind, n.name, n.path, n.line_start);
    }
    Ok(())
}

fn cmd_query_deps(name: String, repo: Option<PathBuf>) -> anyhow::Result<()> {
    let db = GraphDb::open(&resolve_repo(repo))?;
    let all = db.get_all_nodes()?;
    let node_id =
        resolve_id(&all, &name).ok_or_else(|| anyhow::anyhow!("Node not found: {}", name))?;
    let neighbors = db.get_neighbors(&node_id, 1)?;
    for n in neighbors {
        println!("  {}  {:<12}  {}", n.kind, n.name, n.path);
    }
    Ok(())
}

fn cmd_query_blast_radius(name: String, repo: Option<PathBuf>) -> anyhow::Result<()> {
    let db = GraphDb::open(&resolve_repo(repo))?;
    let all = db.get_all_nodes()?;
    let node_id =
        resolve_id(&all, &name).ok_or_else(|| anyhow::anyhow!("Node not found: {}", name))?;
    let neighbors = db.get_neighbors(&node_id, 3)?;
    let count = neighbors.len();
    let risk = if count > 50 {
        "CRITICAL"
    } else if count > 20 {
        "HIGH"
    } else if count > 5 {
        "MEDIUM"
    } else {
        "LOW"
    };
    println!(
        "  Blast radius: {} ({} affected, risk: {})",
        name, count, risk
    );
    for n in neighbors.iter().take(15) {
        println!("    {}  {}", n.kind, n.name);
    }
    Ok(())
}

fn cmd_query_chain(path: String, repo: Option<PathBuf>) -> anyhow::Result<()> {
    let db = GraphDb::open(&resolve_repo(repo))?;
    let parts: Vec<&str> = path.split("->").map(|s| s.trim()).collect();
    if parts.len() != 2 {
        anyhow::bail!("Format: \"<from> -> <to>\"");
    }
    let all = db.get_all_nodes()?;
    let from_id = resolve_id(&all, parts[0])
        .ok_or_else(|| anyhow::anyhow!("From not found: {}", parts[0]))?;
    let to_id =
        resolve_id(&all, parts[1]).ok_or_else(|| anyhow::anyhow!("To not found: {}", parts[1]))?;
    let node_map: std::collections::HashMap<&str, &cgx_engine::Node> =
        all.iter().map(|n| (n.id.as_str(), n)).collect();
    // Build name -> [id] lookup to resolve short callee names in CALLS edges
    let mut name_to_ids: std::collections::HashMap<&str, Vec<&str>> =
        std::collections::HashMap::new();
    for n in &all {
        name_to_ids
            .entry(n.name.as_str())
            .or_default()
            .push(n.id.as_str());
    }
    let edges = db.get_all_edges()?;
    // Build adjacency list, resolving CALLS edge destinations from short names to node IDs
    let mut adj: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for e in &edges {
        let dst_ids: Vec<String> = if node_map.contains_key(e.dst.as_str()) {
            vec![e.dst.clone()]
        } else if let Some(ids) = name_to_ids.get(e.dst.as_str()) {
            ids.iter().map(|s| s.to_string()).collect()
        } else {
            continue;
        };
        for dst in dst_ids {
            adj.entry(e.src.clone()).or_default().push(dst);
        }
    }

    let mut queue = std::collections::VecDeque::new();
    let mut visited = std::collections::HashSet::new();
    let mut parent: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    queue.push_back(from_id.clone());
    visited.insert(from_id.clone());

    while let Some(current) = queue.pop_front() {
        if current == to_id {
            let mut path_nodes = vec![current.clone()];
            let mut cur = current.clone();
            while let Some(p) = parent.get(&cur) {
                path_nodes.push(p.clone());
                cur = p.clone();
            }
            path_nodes.reverse();
            println!("  Chain ({} hops):", path_nodes.len() - 1);
            for (i, id) in path_nodes.iter().enumerate() {
                if let Some(n) = node_map.get(id.as_str()) {
                    println!("    {}. {} ({})", i + 1, n.name, n.kind);
                }
            }
            return Ok(());
        }
        if let Some(nexts) = adj.get(&current) {
            for next in nexts {
                if visited.insert(next.clone()) {
                    parent.insert(next.clone(), current.clone());
                    queue.push_back(next.clone());
                }
            }
        }
    }
    println!("  No call path found from {} to {}", parts[0], parts[1]);
    Ok(())
}

fn cmd_query_owners(path: String, repo: Option<PathBuf>) -> anyhow::Result<()> {
    let db = GraphDb::open(&resolve_repo(repo))?;
    let all_nodes = db.get_all_nodes()?;
    let all_edges = db.get_all_edges()?;
    let file_id = format!("file:{}", path);

    let owners: Vec<_> = all_edges
        .iter()
        .filter(|e| e.kind == "OWNS" && e.dst == file_id)
        .filter_map(|e| all_nodes.iter().find(|n| n.id == e.src))
        .collect();

    if owners.is_empty() {
        println!(
            "No ownership data for {}. Run `cgx analyze` on a git repo first.",
            path
        );
    } else {
        println!("  Ownership for {}:", path);
        for n in owners {
            println!("    {:<24} ({})", n.name, n.id);
        }
    }
    Ok(())
}

fn cmd_query_search(query: String, limit: u32, repo: Option<PathBuf>) -> anyhow::Result<()> {
    let db = GraphDb::open(&resolve_repo(repo))?;
    let all = db.get_all_nodes()?;
    let q = query.to_lowercase();
    for n in all
        .iter()
        .filter(|n| n.name.to_lowercase().contains(&q) || n.path.to_lowercase().contains(&q))
        .take(limit as usize)
    {
        println!("  {}  {:<20}  {}", n.kind, n.name, n.path);
    }
    Ok(())
}

fn cmd_query_community(id: i64, repo: Option<PathBuf>) -> anyhow::Result<()> {
    let db = GraphDb::open(&resolve_repo(repo))?;
    let nodes = db.get_nodes_by_community(id)?;
    let communities = db.get_communities()?;
    let label = communities
        .iter()
        .find(|(cid, ..)| *cid == id)
        .map(|(_, l, _, _)| l.clone())
        .unwrap_or_else(|| format!("community-{}", id));
    println!("  Community #{} — {} ({} nodes)", id, label, nodes.len());
    for n in nodes.iter().take(30) {
        println!("    {}  {:<20}  {}", n.kind, n.name, n.path);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_query_dead_code(
    repo: Option<PathBuf>,
    kind_filter: Option<String>,
    confidence_filter: Option<String>,
    community_filter: Option<i64>,
    path_filter: Option<String>,
    as_json: bool,
    summary_only: bool,
    safe_to_delete: bool,
) -> anyhow::Result<()> {
    use cgx_engine::deadcode::{Confidence, DeadNode, DeadReason};

    let db = GraphDb::open(&resolve_repo(repo))?;
    let report = cgx_engine::detect_dead_code(&db)?;

    let confidence_filter = if safe_to_delete {
        Some("high".to_string())
    } else {
        confidence_filter
    };

    fn filter_items<'a>(
        items: &'a [DeadNode],
        kind_filter: Option<&str>,
        confidence_filter: Option<&str>,
        community_filter: Option<i64>,
        path_filter: Option<&str>,
    ) -> Vec<&'a DeadNode> {
        items
            .iter()
            .filter(|dn| {
                if let Some(k) = kind_filter {
                    let matches = match k {
                        "exports" => dn.reason == DeadReason::UnreferencedExport,
                        "functions" => dn.node.kind == "Function",
                        "variables" => dn.node.kind == "Variable",
                        "files" => dn.reason == DeadReason::ZombieFile,
                        "disconnected" => dn.reason == DeadReason::Disconnected,
                        _ => true,
                    };
                    if !matches {
                        return false;
                    }
                }
                if let Some(c) = confidence_filter {
                    let matches = match c {
                        "high" => dn.confidence == Confidence::High,
                        "medium" => dn.confidence == Confidence::Medium,
                        "low" => dn.confidence == Confidence::Low,
                        _ => true,
                    };
                    if !matches {
                        return false;
                    }
                }
                if let Some(comm) = community_filter {
                    if dn.node.community != comm {
                        return false;
                    }
                }
                if let Some(prefix) = path_filter {
                    if !dn.node.path.starts_with(prefix) {
                        return false;
                    }
                }
                true
            })
            .collect()
    }

    let kf = kind_filter.as_deref();
    let cf = confidence_filter.as_deref();

    let filtered_exports = filter_items(
        &report.unreferenced_exports,
        kf,
        cf,
        community_filter,
        path_filter.as_deref(),
    );
    let filtered_unreachable = filter_items(
        &report.unreachable,
        kf,
        cf,
        community_filter,
        path_filter.as_deref(),
    );
    let filtered_vars = filter_items(
        &report.unused_variables,
        kf,
        cf,
        community_filter,
        path_filter.as_deref(),
    );
    let filtered_disconnected = filter_items(
        &report.disconnected,
        kf,
        cf,
        community_filter,
        path_filter.as_deref(),
    );
    let filtered_zombies = filter_items(
        &report.zombie_files,
        kf,
        cf,
        community_filter,
        path_filter.as_deref(),
    );

    let total = filtered_exports.len()
        + filtered_unreachable.len()
        + filtered_vars.len()
        + filtered_disconnected.len()
        + filtered_zombies.len();

    let (high, medium, low) = {
        let all_filtered: Vec<&DeadNode> = filtered_exports
            .iter()
            .chain(filtered_unreachable.iter())
            .chain(filtered_vars.iter())
            .chain(filtered_disconnected.iter())
            .chain(filtered_zombies.iter())
            .copied()
            .collect();
        let h = all_filtered
            .iter()
            .filter(|dn| dn.confidence == Confidence::High)
            .count();
        let m = all_filtered
            .iter()
            .filter(|dn| dn.confidence == Confidence::Medium)
            .count();
        let l = all_filtered
            .iter()
            .filter(|dn| dn.confidence == Confidence::Low)
            .count();
        (h, m, l)
    };

    if as_json {
        let to_json = |items: &[&DeadNode]| -> serde_json::Value {
            serde_json::Value::Array(
                items
                    .iter()
                    .map(|dn| {
                        serde_json::json!({
                            "id": dn.node.id,
                            "name": dn.node.name,
                            "kind": dn.node.kind,
                            "path": dn.node.path,
                            "line_start": dn.node.line_start,
                            "confidence": dn.confidence.as_str(),
                            "false_positive_risk": dn.false_positive_risk,
                            "churn": dn.node.churn,
                        })
                    })
                    .collect(),
            )
        };
        let output = serde_json::json!({
            "summary": {"total": total, "high": high, "medium": medium, "low": low},
            "unreferenced_exports": to_json(&filtered_exports),
            "unreachable": to_json(&filtered_unreachable),
            "unused_variables": to_json(&filtered_vars),
            "zombie_files": to_json(&filtered_zombies),
            "disconnected": to_json(&filtered_disconnected),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    if summary_only {
        println!(
            "  {:<30} {:>6}  {:>6}  {:>8}  {:>5}",
            "Category", "Count", "High", "Medium", "Low"
        );
        println!("  {}", "-".repeat(65));
        let rows: &[(&str, &[&DeadNode])] = &[
            ("Unreferenced exports", &filtered_exports),
            ("Unreachable functions", &filtered_unreachable),
            ("Unused variables", &filtered_vars),
            ("Disconnected nodes", &filtered_disconnected),
            ("Zombie files", &filtered_zombies),
        ];
        for (label, items) in rows {
            let h = items
                .iter()
                .filter(|dn| dn.confidence == Confidence::High)
                .count();
            let m = items
                .iter()
                .filter(|dn| dn.confidence == Confidence::Medium)
                .count();
            let l = items
                .iter()
                .filter(|dn| dn.confidence == Confidence::Low)
                .count();
            println!(
                "  {:<30} {:>6}  {:>6}  {:>8}  {:>5}",
                label,
                items.len(),
                h,
                m,
                l
            );
        }
        println!("  {}", "-".repeat(65));
        println!(
            "  {:<30} {:>6}  {:>6}  {:>8}  {:>5}",
            "Total", total, high, medium, low
        );
        return Ok(());
    }

    if total == 0 {
        println!("  No dead code detected.");
        return Ok(());
    }

    println!(
        "  Dead code candidates: {} total ({} high, {} medium, {} low confidence)",
        total, high, medium, low
    );
    println!();

    fn print_section(label: &str, items: &[&DeadNode]) {
        if items.is_empty() {
            return;
        }
        println!(
            "  \u{2500}\u{2500} {} ({}) \u{2500}\u{2500}",
            label,
            items.len()
        );
        for dn in items {
            let conf_char = match dn.confidence {
                Confidence::High => "H",
                Confidence::Medium => "M",
                Confidence::Low => "L",
            };
            println!(
                "    [{}] {}  {:<25}  {}:{}",
                conf_char, dn.node.kind, dn.node.name, dn.node.path, dn.node.line_start
            );
            if let Some(ref fp) = dn.false_positive_risk {
                println!("        \u{26A0} {}", fp);
            }
        }
        println!();
    }

    print_section("Unreferenced Exports", &filtered_exports);
    print_section("Unreachable Functions", &filtered_unreachable);
    print_section("Unused Variables", &filtered_vars);
    print_section("Disconnected Nodes", &filtered_disconnected);
    print_section("Zombie Files", &filtered_zombies);

    Ok(())
}

fn cmd_docs_coverage(repo_path: &Path) -> anyhow::Result<()> {
    let canonical = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());
    let db = GraphDb::open(&canonical)?;

    let (overall_pct, by_community, undocumented) = db
        .get_docs_coverage()
        .context("Failed to compute docs coverage")?;

    println!();
    println!("  DOCUMENTATION COVERAGE");
    println!("  {}", "\u{2500}".repeat(60));
    println!(
        "  Overall: {:.1}% of exported functions/classes documented",
        overall_pct
    );
    println!();

    if !by_community.is_empty() {
        println!("  BY COMMUNITY");
        println!(
            "  {:<10}  {:>10}  {:>10}  {:>8}",
            "Community", "Documented", "Total", "Coverage"
        );
        println!("  {}", "\u{2500}".repeat(46));
        for (community, documented, total) in &by_community {
            if *total == 0 {
                continue;
            }
            let pct = (*documented as f64 / *total as f64) * 100.0;
            println!(
                "  {:<10}  {:>10}  {:>10}  {:>7.1}%",
                community, documented, total, pct
            );
        }
        println!();
    }

    if !undocumented.is_empty() {
        println!("  UNDOCUMENTED HIGH-COUPLING FUNCTIONS (top by callers)");
        println!("  {:<30}  {:<35}  {:>7}", "Function", "File", "Callers");
        println!("  {}", "\u{2500}".repeat(76));
        for node in &undocumented {
            println!(
                "  {:<30}  {:<35}  {:>7}",
                truncate_path(&node.name, 30),
                truncate_path(&node.path, 35),
                node.in_degree
            );
        }
        println!();
    }

    Ok(())
}

fn cmd_complexity(
    repo_path: &Path,
    top: usize,
    threshold: Option<f64>,
    combined: bool,
) -> anyhow::Result<()> {
    let canonical = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());
    let db = GraphDb::open(&canonical)?;

    let min_score = threshold.unwrap_or(0.0);
    // When using combined mode, fetch a larger pool so we can re-sort by combined score
    let fetch_limit = if combined { top * 5 } else { top };
    let nodes = db
        .get_nodes_by_complexity(fetch_limit.max(top), min_score)
        .context("Failed to query complexity")?;

    if nodes.is_empty() {
        println!(
            "  No functions found with complexity >= {:.2}. Run `cgx analyze` first.",
            min_score
        );
        return Ok(());
    }

    // Bug 4: warn when all scores are zero (stale index)
    if threshold.is_none() && nodes.iter().all(|n| n.complexity == 0.0) {
        println!(
            "  \u{26a0} All scores are 0.00 \u{2014} run `cgx analyze --force` to compute complexity."
        );
    }

    if combined {
        // Churn lives on File nodes, not Function nodes. Build a path→churn map via
        // a single query joining all File nodes, then multiply complexity × file_churn.
        let file_churn_map: std::collections::HashMap<String, f64> = {
            let mut stmt = db
                .conn
                .prepare("SELECT path, COALESCE(churn, 0.0) FROM nodes WHERE kind = 'File'")?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
            })?;
            rows.filter_map(|r| r.ok()).collect()
        };
        let mut scored: Vec<(f64, &cgx_engine::Node)> = nodes
            .iter()
            .map(|n| {
                let file_churn = file_churn_map.get(&n.path).copied().unwrap_or(0.0);
                (n.complexity * file_churn, n)
            })
            .collect();
        scored.sort_by(|(a, _), (b, _)| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top);

        println!();
        println!(
            "  COMPLEXITY HOTSPOTS \u{2014} top {} functions (combined risk: complexity \u{00d7} file churn)",
            scored.len()
        );
        println!("  {}", "\u{2500}".repeat(80));
        println!(
            "  {:<3}  {:<28}  {:<30}  {:>11}",
            "#", "Function", "File", "CombinedRisk"
        );

        for (i, (combined_score, node)) in scored.iter().enumerate() {
            println!(
                "  {:<3}  {:<28}  {:<30}  {:>11.3}",
                i + 1,
                truncate_path(&node.name, 28),
                truncate_path(&node.path, 30),
                combined_score
            );
        }
    } else {
        println!();
        println!(
            "  COMPLEXITY HOTSPOTS \u{2014} top {} functions",
            nodes.len().min(top)
        );
        println!("  {}", "\u{2500}".repeat(70));
        println!(
            "  {:<3}  {:<28}  {:<30}  {:>8}",
            "#", "Function", "File", "Score"
        );

        for (i, node) in nodes.iter().take(top).enumerate() {
            println!(
                "  {:<3}  {:<28}  {:<30}  {:>7.3}",
                i + 1,
                truncate_path(&node.name, 28),
                truncate_path(&node.path, 30),
                node.complexity
            );
        }
    }

    println!();
    Ok(())
}

fn cmd_dupes(repo_path: &Path, threshold: f64, kind_filter: Option<&str>) -> anyhow::Result<()> {
    let canonical = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());
    let db = GraphDb::open(&canonical)?;

    let all_nodes = db.get_all_nodes().context("Failed to load nodes")?;
    let pairs =
        detect_clones(&all_nodes, &canonical, threshold).context("Failed to detect clones")?;

    // Filter by kind if requested
    let filtered: Vec<&ClonePair> = pairs
        .iter()
        .filter(|p| {
            if let Some(k) = kind_filter {
                p.kind.as_str() == k
            } else {
                true
            }
        })
        .collect();

    if filtered.is_empty() {
        println!(
            "  No duplicate functions found with threshold {:.2}.",
            threshold
        );
        return Ok(());
    }

    println!();
    println!(
        "  DUPLICATE FUNCTIONS \u{2014} {} pair(s) (threshold {:.0}%)",
        filtered.len(),
        threshold * 100.0
    );
    println!("  {}", "\u{2500}".repeat(80));
    println!(
        "  {:<8}  {:<25}  {:<30}  {:<5}  {:<25}  {:<30}",
        "Kind", "Function A", "File A", "Line", "Function B", "File B"
    );
    println!("  {}", "\u{2500}".repeat(130));

    for pair in &filtered {
        println!(
            "  {:<8}  {:<25}  {:<30}  {:<5}  {:<25}  {:<30}  ({:.0}%)",
            pair.kind.as_str(),
            truncate_path(&pair.node_a_name, 25),
            truncate_path(&pair.node_a_path, 30),
            pair.node_a_line,
            truncate_path(&pair.node_b_name, 25),
            truncate_path(&pair.node_b_path, 30),
            pair.similarity * 100.0
        );
    }

    println!();
    Ok(())
}

fn cmd_todos(
    repo_path: &Path,
    tag_filter: Option<&str>,
    kind_filter: Option<&str>,
    as_json: bool,
) -> anyhow::Result<()> {
    let db = GraphDb::open(&resolve_repo(Some(repo_path.to_path_buf())))?;

    let tag_uc = tag_filter.map(|t| t.to_uppercase());
    let tags = db.get_tags(tag_uc.as_deref(), kind_filter)?;

    if as_json {
        println!("{}", serde_json::to_string_pretty(&tags)?);
        return Ok(());
    }

    if tags.is_empty() {
        if tag_filter.is_some() || kind_filter.is_some() {
            let what = tag_filter.or(kind_filter).unwrap_or("annotation");
            println!("  No {} annotation comments found.", what);
        } else {
            println!("  No annotation comments found. Run `cgx analyze` to index the codebase.");
        }
        return Ok(());
    }

    let type_width = tags.iter().map(|t| t.tag_type.len()).max().unwrap_or(5);

    for t in &tags {
        let kind_badge = match t.comment_type.as_str() {
            "jsx" => "[jsx]",
            "jsx_commented_code" => "[jsx-code]",
            _ => "[code]",
        };
        println!(
            "  {:<width$}  {}:{}  {}  {}",
            t.tag_type,
            t.file_path,
            t.line,
            kind_badge,
            t.text.lines().next().unwrap_or("").trim(),
            width = type_width,
        );
    }
    println!();
    println!("  {} annotation(s) found.", tags.len());
    Ok(())
}

// ── Test Coverage ────────────────────────────────────────────────────────

fn cmd_test_coverage(repo_path: &Path, by: Option<&str>) -> anyhow::Result<()> {
    let db = GraphDb::open(&resolve_repo(Some(repo_path.to_path_buf())))?;
    let (pct, tested, untested, _gaps) = db.get_test_coverage_summary(0)?;

    println!("  TEST COVERAGE");
    println!("  {}", "\u{2500}".repeat(60));
    println!(
        "  Overall: {:.1}% of functions/classes have test coverage",
        pct
    );
    println!("  Tested:   {}", tested);
    println!("  Untested: {}", untested);
    println!();

    let tests_edge_count: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM edges WHERE kind = 'TESTS'", [], |r| {
            r.get(0)
        })
        .unwrap_or(0);
    if tests_edge_count == 0 {
        println!("  No TESTS edges found. Add test files (*.test.ts, __tests__/*.ts, etc.)");
        println!("  and run `cgx analyze --force` to detect test coverage.");
    } else {
        println!("  TESTS edges: {}", tests_edge_count);
    }

    if by.map(|b| b == "community").unwrap_or(false) {
        println!();
        println!("  COVERAGE BY COMMUNITY");
        println!("  {}", "\u{2500}".repeat(60));
        println!(
            "  {:<12}  {:>7}  {:>8}  {:>8}  {:>6}",
            "Community", "Total", "Tested", "Untested", "Pct"
        );
        println!("  {}", "\u{2500}".repeat(60));

        let mut stmt = db.conn.prepare(
            "SELECT community, COUNT(*) as total, SUM(CASE WHEN is_tested THEN 1 ELSE 0 END) as tested \
             FROM nodes WHERE kind = 'Function' GROUP BY community ORDER BY total DESC LIMIT 20",
        )?;
        let rows = stmt.query_map([], |row| {
            let community: Option<i64> = row.get(0)?;
            let total: i64 = row.get(1)?;
            let tested: i64 = row.get(2)?;
            Ok((community, total, tested))
        })?;

        for row in rows {
            let (community, total, tested) = row?;
            let untested = total - tested;
            let pct = if total > 0 {
                (tested as f64 / total as f64) * 100.0
            } else {
                0.0
            };
            let community_label = community
                .map(|c| c.to_string())
                .unwrap_or_else(|| "none".to_string());
            println!(
                "  {:<12}  {:>7}  {:>8}  {:>8}  {:>5.1}%",
                community_label, total, tested, untested, pct
            );
        }
        println!();
    }

    Ok(())
}

fn cmd_test_gaps(repo_path: &Path) -> anyhow::Result<()> {
    let db = GraphDb::open(&resolve_repo(Some(repo_path.to_path_buf())))?;
    let (pct, _tested, _untested, gaps) = db.get_test_coverage_summary(20)?;

    println!("  TEST COVERAGE GAPS — ranked by risk (untested × coupling × churn)");
    println!("  {}", "─".repeat(74));

    if gaps.is_empty() {
        println!("  All functions have test coverage! ({:.1}% overall)", pct);
        return Ok(());
    }

    let header = format!(
        "  {:<30}  {:<30}  {:>7}  {:>5}  Risk",
        "Function", "File", "Callers", "Churn"
    );
    println!("{}", header);
    println!("  {}", "─".repeat(74));

    for node in &gaps {
        let risk_score = 1.0 - (1.0 - node.churn) * (1.0 - node.coupling);
        let risk_label = if risk_score > 0.7 {
            "HIGH"
        } else if risk_score > 0.4 {
            "MEDIUM"
        } else {
            "LOW"
        };
        println!(
            "  {:<30}  {:<30}  {:>7}  {:>5.2}  {}",
            node.name, node.path, node.in_degree, node.churn, risk_label
        );
    }
    println!();
    println!("  {} untested function(s) found.", gaps.len());

    Ok(())
}

fn cmd_test_suggest(repo_path: &Path) -> anyhow::Result<()> {
    let db = GraphDb::open(&resolve_repo(Some(repo_path.to_path_buf())))?;
    let (_pct, _tested, _untested, gaps) = db.get_test_coverage_summary(10)?;

    println!("  SUGGESTED TESTS — write these first");
    println!("  {}", "─".repeat(74));

    if gaps.is_empty() {
        println!("  All functions have test coverage!");
        return Ok(());
    }

    for (i, node) in gaps.iter().enumerate() {
        let test_dir = if node.path.contains('/') {
            format!(
                "tests/{}.test.ts",
                node.path.trim_end_matches(".ts").trim_end_matches(".tsx")
            )
        } else {
            format!("tests/{}.test.ts", node.name.to_lowercase())
        };
        println!(
            "  {}. {} ({}:{})",
            i + 1,
            node.name,
            node.path,
            node.line_start
        );
        println!(
            "     Why: {} callers, churn {:.2}, 0 existing tests",
            node.in_degree, node.churn
        );
        println!("     Suggested test file: {}", test_dir);
        println!();
    }

    Ok(())
}

// ── Explain ──────────────────────────────────────────────────────────────

fn cmd_explain(
    repo_path: &Path,
    target: Option<&str>,
    community: Option<i64>,
    onboard: bool,
    out: Option<&Path>,
) -> anyhow::Result<()> {
    let canonical = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());
    let db = GraphDb::open(&canonical)?;

    let output = if onboard {
        generate_onboard_doc(&db)?
    } else if let Some(comm_id) = community {
        generate_community_doc(&db, comm_id)?
    } else if let Some(target_str) = target {
        // Try as symbol name first, then as folder path
        generate_symbol_doc(&db, target_str).or_else(|_| generate_folder_doc(&db, target_str))?
    } else {
        generate_onboard_doc(&db)?
    };

    match out {
        Some(out_path) => {
            std::fs::write(out_path, &output)?;
            println!("  Written to {}", out_path.display());
        }
        None => print!("{}", output),
    }

    Ok(())
}

fn generate_symbol_doc(db: &GraphDb, symbol: &str) -> anyhow::Result<String> {
    let nodes = db.get_all_nodes()?;
    let node = nodes
        .iter()
        .find(|n| n.name == symbol || n.id.contains(symbol))
        .ok_or_else(|| anyhow::anyhow!("Symbol '{}' not found", symbol))?;

    let all_edges = db.get_all_edges()?;
    let callers: Vec<_> = all_edges
        .iter()
        .filter(|e| e.dst == node.id && matches!(e.kind.as_str(), "CALLS" | "TESTS"))
        .collect();
    let callees: Vec<_> = all_edges
        .iter()
        .filter(|e| e.src == node.id && e.kind == "CALLS")
        .collect();

    // Get tags for this file
    let tags = db.get_tags(None, None)?;
    let file_tags: Vec<_> = tags.iter().filter(|t| t.file_path == node.path).collect();

    let mut out = String::new();
    out.push_str(&format!("## {}\n\n", node.name));
    out.push_str(&format!(
        "**Kind:** {}  **File:** {}:{}–{}  **Community:** #{}\n",
        node.kind, node.path, node.line_start, node.line_end, node.community
    ));
    out.push_str(&format!(
        "**Churn:** {:.2}  **Coupling:** {:.2}  **Complexity:** {:.2}\n\n",
        node.churn, node.coupling, node.complexity
    ));

    if !callers.is_empty() {
        out.push_str(&format!(
            "**What depends on it ({} callers):**\n",
            callers.len()
        ));
        for caller in callers.iter().take(10) {
            if let Some(caller_node) = nodes.iter().find(|n| n.id == caller.src) {
                out.push_str(&format!(
                    "- {} ({}: {}:{})\n",
                    caller_node.name, caller_node.kind, caller_node.path, caller_node.line_start
                ));
            }
        }
        out.push('\n');
    }

    if !callees.is_empty() {
        out.push_str(&format!(
            "**What it depends on ({} dependencies):**\n",
            callees.len()
        ));
        for callee in callees.iter().take(10) {
            if let Some(callee_node) = nodes.iter().find(|n| n.id == callee.dst) {
                out.push_str(&format!(
                    "- {} ({}: {}:{})\n",
                    callee_node.name, callee_node.kind, callee_node.path, callee_node.line_start
                ));
            }
        }
        out.push('\n');
    }

    if !file_tags.is_empty() {
        out.push_str("**Open TODOs:**\n");
        for tag in &file_tags {
            out.push_str(&format!(
                "- Line {}: {} ({})\n",
                tag.line,
                tag.text.lines().next().unwrap_or("").trim(),
                tag.tag_type
            ));
        }
        out.push('\n');
    }

    Ok(out)
}

fn generate_folder_doc(db: &GraphDb, folder_path: &str) -> anyhow::Result<String> {
    let folder = folder_path.trim_end_matches('/');
    let nodes = db.get_all_nodes()?;
    let folder_nodes: Vec<_> = nodes
        .iter()
        .filter(|n| n.path.starts_with(folder) && n.kind != "Author")
        .collect();

    if folder_nodes.is_empty() {
        return Err(anyhow::anyhow!("No nodes found under '{}'", folder));
    }

    let mut out = String::new();
    out.push_str(&format!("## Folder: {}/\n\n", folder));
    out.push_str(&format!("**Total nodes:** {}\n\n", folder_nodes.len()));

    // Show exported symbols
    let exported: Vec<_> = folder_nodes
        .iter()
        .filter(|n| n.exported && matches!(n.kind.as_str(), "Function" | "Class"))
        .collect();
    if !exported.is_empty() {
        out.push_str("**Exported symbols:**\n");
        for node in exported.iter().take(15) {
            out.push_str(&format!(
                "- {} `{}` ({}:{}) — {} callers\n",
                node.kind, node.name, node.path, node.line_start, node.in_degree
            ));
        }
        out.push('\n');
    }

    // Top 3 hotspots
    let mut hotspots: Vec<_> = folder_nodes.iter().filter(|n| n.churn > 0.0).collect();
    hotspots.sort_by(|a, b| {
        let score_a = a.churn * a.coupling;
        let score_b = b.churn * b.coupling;
        score_b
            .partial_cmp(&score_a)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if !hotspots.is_empty() {
        out.push_str("**Top 3 hotspots:**\n");
        for node in hotspots.iter().take(3) {
            out.push_str(&format!(
                "- `{}` — churn {:.2}, coupling {:.2}\n",
                node.name, node.churn, node.coupling
            ));
        }
        out.push('\n');
    }

    Ok(out)
}

fn generate_community_doc(db: &GraphDb, community_id: i64) -> anyhow::Result<String> {
    let nodes = db.get_nodes_by_community(community_id)?;
    if nodes.is_empty() {
        return Err(anyhow::anyhow!("Community #{} not found", community_id));
    }

    let mut out = String::new();
    out.push_str(&format!("## Community #{}\n\n", community_id));
    out.push_str(&format!("**Nodes:** {}\n\n", nodes.len()));

    // Find the "center" file (most common path)
    let mut path_counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for n in &nodes {
        *path_counts.entry(&n.path).or_default() += 1;
    }
    if let Some((main_path, _)) = path_counts.iter().max_by_key(|(_, c)| *c) {
        out.push_str(&format!("**Main file:** {}\n\n", main_path));
    }

    out.push_str("**Top nodes by callers:**\n");
    let mut sorted = nodes.clone();
    sorted.sort_by_key(|n| std::cmp::Reverse(n.in_degree));
    for node in sorted.iter().take(10) {
        out.push_str(&format!(
            "- {} `{}` — {} callers\n",
            node.kind, node.name, node.in_degree
        ));
    }

    Ok(out)
}

fn generate_onboard_doc(db: &GraphDb) -> anyhow::Result<String> {
    let stats = db.get_stats()?;
    let all_edges = db.get_all_edges()?;

    let mut out = String::new();
    out.push_str("# Architecture Overview\n\n");

    // Section 1: Overview
    out.push_str("## 1. Repository Overview\n\n");
    let lang_list: Vec<String> = stats
        .language_breakdown
        .iter()
        .filter(|(_, pct)| **pct > 0.01)
        .map(|(lang, pct)| format!("{} ({:.0}%)", lang, pct * 100.0))
        .collect();
    out.push_str(&format!(
        "- **Languages:** {}\n",
        if lang_list.is_empty() {
            "unknown".to_string()
        } else {
            lang_list.join(", ")
        }
    ));
    out.push_str(&format!(
        "- **Nodes:** {} total ({} functions, {} classes, {} files)\n",
        stats.node_count, stats.function_count, stats.class_count, stats.file_count
    ));
    out.push_str(&format!("- **Edges:** {}\n", stats.edge_count));
    out.push_str(&format!("- **Communities:** {}\n\n", stats.community_count));

    // Section 2: Entry Points
    out.push_str("## 2. Entry Points\n\n");
    let entry_points = db.get_entry_points(5)?;
    if entry_points.is_empty() {
        out.push_str("No clear entry points detected.\n\n");
    } else {
        for ep in &entry_points {
            out.push_str(&format!(
                "- `{}` ({}) — {} outgoing calls\n",
                ep.name, ep.path, ep.out_degree
            ));
        }
        out.push('\n');
    }

    // Section 3: Most Important Symbols
    out.push_str("## 3. Most Important Symbols\n\n");
    let god_nodes = db.get_god_nodes(5)?;
    for gn in &god_nodes {
        out.push_str(&format!(
            "- `{}` ({}) in `{}` — {} callers\n",
            gn.name, gn.kind, gn.path, gn.in_degree
        ));
    }
    out.push('\n');

    // Section 4: Hotspots
    out.push_str("## 4. Hotspots (High Risk Files)\n\n");
    let hotspots = db.get_hotspots(5)?;
    if hotspots.is_empty() {
        out.push_str("No hotspots detected (may need git history).\n\n");
    } else {
        for (path, churn, coupling, callers) in &hotspots {
            out.push_str(&format!(
                "- `{}` — churn {:.2}, coupling {:.2}, {} callers\n",
                path, churn, coupling, callers
            ));
        }
        out.push('\n');
    }

    // Section 5: Technical Debt (TODOs)
    out.push_str("## 5. Technical Debt\n\n");
    let tags = db.get_tags(None, None)?;
    if tags.is_empty() {
        out.push_str("No TODO/FIXME comments found.\n\n");
    } else {
        for tag in tags.iter().take(10) {
            out.push_str(&format!(
                "- **{}** in `{}` line {}: {}\n",
                tag.tag_type,
                tag.file_path,
                tag.line,
                tag.text.lines().next().unwrap_or("").trim()
            ));
        }
        out.push('\n');
    }

    // Section 6: Ownership
    out.push_str("## 6. Code Ownership\n\n");
    let owners = db.get_ownership()?;
    if owners.is_empty() {
        out.push_str("No ownership data (may need git history).\n\n");
    } else {
        for (owner, files) in owners.iter().take(5) {
            out.push_str(&format!("- {} owns {} file(s)\n", owner, files));
        }
        out.push('\n');
    }

    // Section 7: Test Coverage Gaps
    out.push_str("## 7. Testing Gaps\n\n");
    let (pct, _tested, _untested, gaps) = db.get_test_coverage_summary(5)?;
    out.push_str(&format!("Overall test coverage: {:.1}%\n\n", pct));
    if gaps.is_empty() {
        out.push_str("No obvious test gaps detected.\n\n");
    } else {
        let tests_edges: usize = all_edges.iter().filter(|e| e.kind == "TESTS").count();
        if tests_edges == 0 {
            out.push_str("No test files detected. Add test files to enable coverage analysis.\n\n");
        } else {
            for gap in &gaps {
                out.push_str(&format!(
                    "- `{}` ({}) — {} callers, no test coverage\n",
                    gap.name, gap.path, gap.in_degree
                ));
            }
            out.push('\n');
        }
    }

    Ok(out)
}

// ── Architecture Rules ───────────────────────────────────────────────────

fn cmd_rules_check(
    repo_path: &Path,
    rule_filter: Option<&str>,
    format: &str,
) -> anyhow::Result<()> {
    let canonical = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());
    let db = GraphDb::open(&canonical)?;
    let config = cgx_engine::RulesConfig::load(&canonical)?;

    if config.rules.is_empty() {
        println!("  No rules defined. Create .cgx/rules.toml to add architecture rules.");
        println!("  See: cgx rules list for examples.");
        return Ok(());
    }

    let results = cgx_engine::run_rules(&db, &config.rules, rule_filter);

    let error_count = results
        .iter()
        .filter(|r| !r.passed() && r.rule.severity == "error")
        .count();
    let warning_count = results
        .iter()
        .filter(|r| !r.passed() && r.rule.severity == "warning")
        .count();

    match format {
        "github-actions" => {
            for result in &results {
                for v in &result.violations {
                    let level = if v.severity == "error" {
                        "error"
                    } else {
                        "warning"
                    };
                    let file_part = v
                        .file
                        .as_deref()
                        .map(|f| format!("file={},", f))
                        .unwrap_or_default();
                    println!(
                        "::{} {}title=cgx Rule {}::{}",
                        level, file_part, v.rule_name, v.message
                    );
                }
            }
        }
        _ => {
            println!(
                "  ARCHITECTURE RULES \u{2014} {} defined",
                config.rules.len()
            );
            println!("  {}", "\u{2500}".repeat(70));

            for result in &results {
                let icon = if result.error.is_some() {
                    "?"
                } else if result.passed() {
                    "\u{2713}"
                } else {
                    "\u{2717}"
                };
                let severity_label = if !result.passed() {
                    format!(" {}", result.rule.severity.to_uppercase())
                } else {
                    String::new()
                };

                println!(
                    "  {}  {:<50} ({} violations){}",
                    icon,
                    result.rule.name,
                    result.violations.len(),
                    severity_label
                );

                if let Some(ref err) = result.error {
                    println!("    ERROR: {}", err);
                }

                for v in result.violations.iter().take(5) {
                    let file_prefix = v
                        .file
                        .as_deref()
                        .map(|f| format!("{}: ", f))
                        .unwrap_or_default();
                    println!("    {}{}", file_prefix, v.message);
                }
            }

            println!();
            if error_count > 0 {
                println!(
                    "  Result: FAIL ({} error(s), {} warning(s))",
                    error_count, warning_count
                );
            } else if warning_count > 0 {
                println!("  Result: PASS with {} warning(s)", warning_count);
            } else {
                println!("  Result: PASS \u{2014} all rules satisfied");
            }
        }
    }

    if error_count > 0 {
        std::process::exit(1);
    }

    Ok(())
}

fn cmd_rules_list(repo_path: &Path) -> anyhow::Result<()> {
    let canonical = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());
    let config = cgx_engine::RulesConfig::load(&canonical)?;

    if config.rules.is_empty() {
        println!("  No user rules defined. Create .cgx/rules.toml to add custom rules.");
        println!();
        println!("  AVAILABLE BUILT-IN RULES");
        println!("  {}", "\u{2500}".repeat(54));
        println!(
            "  [built-in]  {:<26}  \u{2014} Detect circular dependencies between modules",
            "no_cycles"
        );
        println!(
            "  [built-in]  {:<26}  \u{2014} Flag nodes with in-degree above threshold (default 50)",
            "max_coupling"
        );
        println!(
            "  [built-in]  {:<26}  \u{2014} Flag functions with complexity above threshold (default 0.7)",
            "max_complexity"
        );
        println!(
            "  [built-in]  {:<26}  \u{2014} Require doc comments on exported functions",
            "require_docs_for_public"
        );
        println!();
        println!("  Example .cgx/rules.toml:");
        println!("  [[rules]]");
        println!("  name = \"no-circular-deps\"");
        println!("  built_in = \"no_cycles\"");
        println!("  severity = \"error\"");
        return Ok(());
    }

    println!("  RULES ({})", config.rules.len());
    println!("  {}", "\u{2500}".repeat(70));
    for rule in &config.rules {
        let kind = if rule.built_in.is_some() {
            "built-in"
        } else {
            "sql"
        };
        println!("  [{:<8}] [{:<8}] {}", kind, rule.severity, rule.name);
        if !rule.description.is_empty() {
            println!("             {}", rule.description);
        }
    }

    Ok(())
}

// ── PR Review ────────────────────────────────────────────────────────────

fn cmd_review(repo_path: &Path, commit_ref: &str, format: &str) -> anyhow::Result<()> {
    let canonical = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());

    let git_repo = git2::Repository::open(&canonical)
        .context("Not a git repository. Run cgx analyze first.")?;

    // Parse commit range
    let (old_commit, new_commit) = resolve_commit_range(&git_repo, commit_ref)?;

    // Get diff between old and new trees
    let old_tree = old_commit.tree().context("Failed to get old tree")?;
    let new_tree = new_commit.tree().context("Failed to get new tree")?;

    let diff = git_repo
        .diff_tree_to_tree(Some(&old_tree), Some(&new_tree), None)
        .context("Failed to compute diff")?;

    let mut changed_paths: Vec<String> = Vec::new();
    diff.foreach(
        &mut |delta, _| {
            if let Some(path) = delta.new_file().path().and_then(|p| p.to_str()) {
                changed_paths.push(path.to_string());
            }
            true
        },
        None,
        None,
        None,
    )?;

    // Get ownership info
    let db = GraphDb::open(&canonical)?;

    // Find changed nodes
    let all_nodes = db.get_all_nodes()?;
    let changed_nodes: Vec<&Node> = all_nodes
        .iter()
        .filter(|n| changed_paths.contains(&n.path) && n.kind != "Author")
        .collect();

    // Find suggested reviewers via OWNS edges
    let owners = db.get_ownership()?;
    let file_owners: Vec<String> = owners
        .iter()
        .filter(|(name, _)| !name.is_empty())
        .take(3)
        .map(|(name, _)| name.clone())
        .collect();

    // Find blast radius — nodes dependent on changed nodes
    let mut blast_set: std::collections::HashSet<String> = std::collections::HashSet::new();
    for node in &changed_nodes {
        if let Ok(neighbors) = db.get_neighbors(&node.id, 2) {
            for n in neighbors {
                if !changed_paths.contains(&n.path) {
                    blast_set.insert(n.id.clone());
                }
            }
        }
    }

    // Find hotspot alerts
    let hotspot_alerts: Vec<&Node> = changed_nodes
        .iter()
        .filter(|n| n.churn > 0.7 && n.coupling > 0.6)
        .copied()
        .collect();

    // Find missing tests
    let missing_tests: Vec<&Node> = changed_nodes
        .iter()
        .filter(|n| matches!(n.kind.as_str(), "Function" | "Class") && !n.is_tested)
        .copied()
        .collect();

    // Find doc gaps
    let doc_gaps: Vec<&Node> = changed_nodes
        .iter()
        .filter(|n| matches!(n.kind.as_str(), "Function" | "Class"))
        .copied()
        .collect();

    // Find TODOs in changed files
    let tags = db.get_tags(None, None)?;
    let open_todos: Vec<&TagRow> = tags
        .iter()
        .filter(|t| changed_paths.contains(&t.file_path))
        .collect();

    let risk_level = if !hotspot_alerts.is_empty() || blast_set.len() > 20 {
        "HIGH"
    } else if blast_set.len() > 5 || !missing_tests.is_empty() {
        "MEDIUM"
    } else {
        "LOW"
    };

    match format {
        "markdown" => {
            println!("## cgx Review Brief");
            println!();
            println!("**Risk Level:** {}", risk_level);
            println!("**Changed Nodes:** {}", changed_nodes.len());
            println!("**Blast Radius:** {} downstream nodes", blast_set.len());
            println!();
            println!("### Changed Nodes");
            for node in &changed_nodes {
                println!("- `{}` ({}) — {}", node.name, node.kind, node.path);
            }
            if !hotspot_alerts.is_empty() {
                println!();
                println!("### ⚠️ Hotspot Alerts");
                for node in &hotspot_alerts {
                    println!(
                        "- `{}` — churn {:.2}, coupling {:.2}",
                        node.name, node.churn, node.coupling
                    );
                }
            }
            if !missing_tests.is_empty() {
                println!();
                println!("### Missing Tests");
                for node in &missing_tests {
                    println!("- `{}` ({}) — no test coverage", node.name, node.path);
                }
            }
            if !file_owners.is_empty() {
                println!();
                println!("### Suggested Reviewers");
                for owner in &file_owners {
                    println!("- {}", owner);
                }
            }
            if !doc_gaps.is_empty() {
                println!();
                println!("### Documentation Gaps");
                for node in &doc_gaps {
                    println!("- `{}` — no doc comment", node.name);
                }
            }
            if !open_todos.is_empty() {
                println!();
                println!("### Open TODOs in Changed Files");
                for tag in open_todos.iter().take(5) {
                    println!(
                        "- {} in {} line {}: {}",
                        tag.tag_type, tag.file_path, tag.line, tag.text
                    );
                }
            }
        }
        "github-actions" => {
            for node in &hotspot_alerts {
                println!(
                    "::warning file={},line={},title=cgx Hotspot Alert::{} has churn {:.2} — high risk modification",
                    node.path, node.line_start, node.name, node.churn
                );
            }
            for node in &missing_tests {
                println!(
                    "::warning file={},line={},title=cgx Missing Test::{} has {} callers and no tests",
                    node.path, node.line_start, node.name, node.in_degree
                );
            }
            if !blast_set.is_empty() {
                for node in changed_nodes.iter().take(3) {
                    println!(
                        "::error file={},title=cgx Blast Radius::Changing {} affects {} nodes",
                        node.path,
                        node.name,
                        blast_set.len()
                    );
                }
            }
        }
        _ => {
            // text format
            println!("  CGX REVIEW BRIEF");
            println!("  Ref: {}", commit_ref);
            println!("  Risk Level: {}", risk_level);
            println!();
            println!("  CHANGED NODES ({} total)", changed_nodes.len());
            println!("  {}", "\u{2500}".repeat(60));
            for node in &changed_nodes {
                println!("  {} {} — {}", node.kind, node.name, node.path);
            }
            println!();
            println!(
                "  BLAST RADIUS — {} downstream node(s) affected",
                blast_set.len()
            );
            if !hotspot_alerts.is_empty() {
                println!();
                println!("  HOTSPOT ALERTS ({} file(s))", hotspot_alerts.len());
                println!("  {}", "\u{2500}".repeat(60));
                for node in &hotspot_alerts {
                    println!(
                        "  {} — churn {:.2}, coupling {:.2}",
                        node.name, node.churn, node.coupling
                    );
                }
            }
            if !missing_tests.is_empty() {
                println!();
                println!("  MISSING TESTS ({} function(s))", missing_tests.len());
                for node in &missing_tests {
                    println!("  {} ({}) — no test coverage", node.name, node.path);
                }
            }
            println!();
            println!("  SUGGESTED REVIEWERS");
            println!("  {}", "\u{2500}".repeat(60));
            if file_owners.is_empty() {
                println!("  (no ownership data — run with git history)");
            } else {
                for owner in &file_owners {
                    println!("  {}", owner);
                }
            }
            if !open_todos.is_empty() {
                println!();
                println!("  OPEN TODOS ({} in changed files)", open_todos.len());
                for tag in open_todos.iter().take(5) {
                    println!(
                        "  {} {}:{} — {}",
                        tag.tag_type, tag.file_path, tag.line, tag.text
                    );
                }
            }
        }
    }

    Ok(())
}

fn resolve_commit_range<'repo>(
    repo: &'repo git2::Repository,
    spec: &str,
) -> anyhow::Result<(git2::Commit<'repo>, git2::Commit<'repo>)> {
    // Support HEAD~N, branch name, or commit SHA
    if let Some((old_spec, new_spec)) = spec.split_once("..") {
        let old_obj = repo
            .revparse_single(old_spec)
            .with_context(|| format!("Cannot resolve ref: {}", old_spec))?;
        let new_obj = repo
            .revparse_single(new_spec)
            .with_context(|| format!("Cannot resolve ref: {}", new_spec))?;
        let old_commit = old_obj
            .peel_to_commit()
            .context("Old ref is not a commit")?;
        let new_commit = new_obj
            .peel_to_commit()
            .context("New ref is not a commit")?;
        Ok((old_commit, new_commit))
    } else {
        // Treat as "old_spec to HEAD"
        let old_obj = repo
            .revparse_single(spec)
            .with_context(|| format!("Cannot resolve ref: {}", spec))?;
        let new_obj = repo
            .revparse_single("HEAD")
            .context("Cannot resolve HEAD")?;
        let old_commit = old_obj.peel_to_commit().context("Ref is not a commit")?;
        let new_commit = new_obj.peel_to_commit().context("HEAD is not a commit")?;
        Ok((old_commit, new_commit))
    }
}

// ── Dependency Health ────────────────────────────────────────────────────

fn cmd_deps_health(repo_path: &Path, critical_only: bool) -> anyhow::Result<()> {
    let canonical = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());

    println!("  Querying OSV vulnerability database...");
    let reports = cgx_engine::audit_dependencies(&canonical)?;

    if reports.is_empty() {
        println!(
            "  No package manifests found (package.json, Cargo.toml, requirements.txt, go.mod)."
        );
        return Ok(());
    }

    let filtered: Vec<_> = if critical_only {
        reports.iter().filter(|r| r.cve_count > 0).collect()
    } else {
        reports.iter().collect()
    };

    let total_cves: i64 = reports.iter().map(|r| r.cve_count).sum();
    let affected: usize = reports.iter().filter(|r| r.cve_count > 0).count();

    println!();
    println!("  DEPENDENCY HEALTH \u{2014} {} packages", reports.len());
    println!("  {}", "\u{2500}".repeat(74));
    println!(
        "  {:<30}  {:<12}  {:>5}  {:>8}  Risk",
        "Package", "Version", "CVEs", "Ecosystem"
    );
    println!("  {}", "\u{2500}".repeat(74));

    for r in &filtered {
        let risk = if r.cve_count >= 3 {
            "CRITICAL"
        } else if r.cve_count >= 1 {
            "HIGH"
        } else {
            "LOW"
        };
        println!(
            "  {:<30}  {:<12}  {:>5}  {:>8}  {}",
            r.name, r.version, r.cve_count, r.ecosystem, risk
        );
    }

    println!();
    if total_cves > 0 {
        println!(
            "  {} package(s) with {} total CVE(s) found.",
            affected, total_cves
        );
    } else {
        println!("  No known CVEs in your dependencies.");
    }

    Ok(())
}

fn cmd_deps_audit(repo_path: &Path) -> anyhow::Result<()> {
    let canonical = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());

    let reports = cgx_engine::audit_dependencies(&canonical)?;

    if reports.is_empty() {
        println!("  No package manifests found.");
        return Ok(());
    }

    let total_cves: i64 = reports.iter().map(|r| r.cve_count).sum();

    println!("  DEPENDENCY AUDIT");
    println!("  {}", "\u{2500}".repeat(50));
    println!("  Packages scanned: {}", reports.len());
    println!("  Total CVEs found: {}", total_cves);

    if total_cves > 0 {
        println!();
        println!("  Vulnerable packages:");
        for r in reports.iter().filter(|r| r.cve_count > 0) {
            println!(
                "    {} {} \u{2014} {} CVE(s)",
                r.name, r.version, r.cve_count
            );
            for cve in r.cve_ids.iter().take(3) {
                println!("      {}", cve);
            }
        }
    } else {
        println!("  No known CVEs in your dependencies.");
    }

    Ok(())
}

fn cmd_deps_outdated(repo_path: &Path) -> anyhow::Result<()> {
    let canonical = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());

    let deps = cgx_engine::parse_manifests(&canonical)?;

    if deps.is_empty() {
        println!("  No package manifests found.");
        return Ok(());
    }

    println!("  OUTDATED PACKAGES");
    println!("  {}", "\u{2500}".repeat(50));
    println!("  {} packages found in manifest(s).", deps.len());
    println!();
    println!("  (Version comparison requires registry access.)");
    println!("  Run npm outdated / cargo outdated / pip list --outdated for precise results.");

    Ok(())
}

// ── GitHub Pages Publisher ───────────────────────────────────────────────

fn cmd_publish(repo_path: &Path, dry_run: bool, badge: bool) -> anyhow::Result<()> {
    let canonical = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());

    // Determine GitHub Pages URL from remote
    let repo = git2::Repository::open(&canonical);
    let remote_url = repo.as_ref().ok().and_then(|r| {
        r.find_remote("origin")
            .ok()
            .and_then(|remote| remote.url().map(|s| s.to_string()))
    });

    let pages_url = remote_url.as_ref().and_then(|url| {
        if let Some(rest) = url.strip_prefix("https://github.com/") {
            let path = rest.strip_suffix(".git").unwrap_or(rest);
            Some(format!(
                "https://{}.github.io/{}",
                path.split('/').next()?,
                path.split('/').nth(1)?
            ))
        } else if let Some(rest) = url.strip_prefix("git@github.com:") {
            let path = rest.strip_suffix(".git").unwrap_or(rest);
            Some(format!(
                "https://{}.github.io/{}",
                path.split('/').next()?,
                path.split('/').nth(1)?
            ))
        } else {
            None
        }
    });

    if badge {
        println!();
        println!("  Add this badge to your README.md:");
        println!();
        if let Some(ref url) = pages_url {
            println!(
                "  [![cgx graph](https://img.shields.io/badge/cgx-graph-blue)]({})",
                url
            );
            println!();
            println!("  Your graph will be published at: {}", url);
        } else {
            println!("  [![cgx graph](https://img.shields.io/badge/cgx-graph-blue)](https://AayushBahukhandi.github.io/cgx/)");
            println!();
            println!("  (Replace the URL with your actual GitHub Pages URL)");
        }
        println!();
        return Ok(());
    }

    // Step 1: Extract embedded web UI assets to a temp directory
    let tmp_dir = std::env::temp_dir().join("cgx-publish-ui");
    if tmp_dir.exists() {
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
    std::fs::create_dir_all(&tmp_dir)?;
    for file_path in WebUiAssets::iter() {
        let file_data = WebUiAssets::get(file_path.as_ref())
            .ok_or_else(|| anyhow::anyhow!("Embedded asset missing: {}", file_path))?;
        let dest = tmp_dir.join(file_path.as_ref());
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&dest, file_data.data)?;
    }
    let dist_dir = tmp_dir;

    // Step 2: Generate graph JSON
    eprintln!("  Generating graph JSON...");
    let db = GraphDb::open(&canonical)?;
    if db.node_count()? == 0 {
        anyhow::bail!("No indexed graph. Run `cgx analyze` first.");
    }
    let graph_json = cgx_engine::export_json(&db)?;

    let index_path = dist_dir.join("index.html");
    if !index_path.exists() {
        anyhow::bail!("dist/index.html not found after copy");
    }

    // Step 4: Inject graph data into index.html
    eprintln!("  Injecting graph data...");
    let mut html = std::fs::read_to_string(&index_path)?;
    let inject_script = format!("<script>window.__CGX_GRAPH__ = {};</script>", graph_json);
    // Insert before closing </head> or </body>
    if let Some(pos) = html.find("</head>") {
        html.insert_str(pos, &format!("{}\n  ", inject_script));
    } else if let Some(pos) = html.find("</body>") {
        html.insert_str(pos, &format!("{}\n  ", inject_script));
    } else {
        anyhow::bail!("Could not find </head> or </body> in index.html");
    }
    std::fs::write(&index_path, &html)?;

    if dry_run {
        println!();
        println!("  PUBLISH — dry run");
        println!("  {}", "\u{2500}".repeat(50));
        println!("  Repo:         {}", canonical.display());
        println!("  Graph nodes:  {}", db.node_count()?);
        println!("  Graph edges:  {}", db.edge_count()?);
        println!("  Dist dir:     {}", dist_dir.display());
        if let Some(ref url) = pages_url {
            println!("  Live URL:     {}", url);
        } else {
            println!("  Live URL:     (could not determine — not a GitHub remote)");
        }
        println!();
        println!("  Would force-push dist/ to gh-pages branch");
        return Ok(());
    }

    // Step 5: Push to gh-pages
    eprintln!("  Pushing to gh-pages...");
    push_to_gh_pages(&canonical, &dist_dir)?;

    println!();
    println!("  \u{2713} Graph published to GitHub Pages");
    if let Some(ref url) = pages_url {
        println!();
        println!("  Live URL:   {}", url);
    }
    Ok(())
}

async fn cmd_share(repo_path: &Path, token: Option<&str>, public: bool) -> anyhow::Result<()> {
    let canonical = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());

    // Resolve GitHub token: --token > GITHUB_TOKEN > gh CLI
    let gh_token = token
        .map(|t| t.to_string())
        .or_else(|| std::env::var("GITHUB_TOKEN").ok())
        .or_else(|| {
            std::process::Command::new("gh")
                .args(["auth", "token"])
                .output()
                .ok()
                .and_then(|o| {
                    if o.status.success() {
                        String::from_utf8(o.stdout)
                            .ok()
                            .map(|s| s.trim().to_string())
                    } else {
                        None
                    }
                })
        })
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No GitHub token found.\n  Set GITHUB_TOKEN, pass --token, or run `gh auth login`."
            )
        })?;

    eprintln!("  Generating graph...");
    let db = GraphDb::open(&canonical)?;
    if db.node_count()? == 0 {
        anyhow::bail!("No indexed graph. Run `cgx analyze` first.");
    }
    let graph_json = cgx_engine::export_json(&db)?;

    let repo_name = canonical
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "repo".to_string());

    eprintln!("  Uploading graph to GitHub Gist...");

    let client = reqwest::Client::builder().user_agent("cgx-cli").build()?;

    // Check if a cgx Gist already exists for this repo (description match)
    let description = format!("cgx graph — {}", repo_name);

    // Create or update Gist
    let body = serde_json::json!({
        "description": description,
        "public": public,
        "files": {
            "cgx-graph.json": {
                "content": graph_json
            }
        }
    });

    let resp = client
        .post("https://api.github.com/gists")
        .header("Authorization", format!("token {}", gh_token))
        .header("Accept", "application/vnd.github+json")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("GitHub API error {}: {}", status, text);
    }

    let gist: serde_json::Value = resp.json().await?;
    let gist_id = gist["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No gist id in response"))?;
    let owner = gist["owner"]["login"].as_str().unwrap_or("unknown");

    let raw_url = format!(
        "https://gist.githubusercontent.com/{}/{}/raw/cgx-graph.json",
        owner, gist_id
    );

    // The hosted viewer is the cgx GitHub Pages site — it reads ?data= to load remote JSON
    let viewer_url = format!(
        "https://AayushBahukhandi.github.io/cgx/?data={}",
        urlenccode(&raw_url)
    );

    println!();
    println!("  \u{2713} Graph shared!");
    println!();
    println!("  Viewer URL (share this):");
    println!("  {}", viewer_url);
    println!();
    println!("  Raw JSON:  {}", raw_url);
    println!("  Gist:      https://gist.github.com/{}/{}", owner, gist_id);
    if !public {
        println!();
        println!("  (secret Gist — only people with the URL can view it)");
    }
    println!();
    println!("  To unshare:  gh gist delete {}", gist_id);
    println!();
    Ok(())
}

fn urlenccode(s: &str) -> String {
    s.chars().fold(String::new(), |mut acc, c| {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => acc.push(c),
            _ => {
                for byte in c.to_string().as_bytes() {
                    acc.push_str(&format!("%{:02X}", byte));
                }
            }
        }
        acc
    })
}

fn push_to_gh_pages(repo_path: &Path, dist_dir: &Path) -> anyhow::Result<()> {
    let repo = git2::Repository::open(repo_path)
        .context("Failed to open git repo. Make sure you're in a git repository.")?;

    let mut remote = repo
        .find_remote("origin")
        .context("No 'origin' remote found. Add a GitHub remote first.")?;

    // Build tree from dist files
    let mut tree_builder = repo.treebuilder(None)?;
    collect_files(dist_dir, &mut tree_builder, &repo)?;
    let tree_oid = tree_builder.write()?;
    let tree = repo.find_tree(tree_oid)?;

    // Get or create gh-pages ref
    let signature = repo
        .signature()
        .context("Git user config not set. Run: git config user.name / user.email")?;

    let parent_commit = repo
        .find_reference("refs/heads/gh-pages")
        .ok()
        .and_then(|r| r.peel_to_commit().ok());

    let _commit_oid = if let Some(parent) = parent_commit {
        repo.commit(
            Some("refs/heads/gh-pages"),
            &signature,
            &signature,
            "cgx publish — update graph",
            &tree,
            &[&parent],
        )?
    } else {
        repo.commit(
            Some("refs/heads/gh-pages"),
            &signature,
            &signature,
            "cgx publish — initial graph",
            &tree,
            &[],
        )?
    };

    // Force push
    let refspec = "+refs/heads/gh-pages:refs/heads/gh-pages";
    let mut push_opts = git2::PushOptions::new();
    let mut callbacks = git2::RemoteCallbacks::new();

    callbacks.credentials(|_url, username, allowed_types| {
        if allowed_types.contains(git2::CredentialType::SSH_KEY) {
            git2::Cred::ssh_key_from_agent(username.unwrap_or("git"))
        } else if allowed_types.contains(git2::CredentialType::USER_PASS_PLAINTEXT) {
            git2::Cred::credential_helper(&git2::Config::open_default()?, _url, username)
        } else {
            git2::Cred::default()
        }
    });

    push_opts.remote_callbacks(callbacks);
    remote
        .push(&[refspec], Some(&mut push_opts))
        .context("Failed to push to gh-pages. Check your GitHub credentials.")?;

    Ok(())
}

fn collect_files(
    dir: &Path,
    tree_builder: &mut git2::TreeBuilder,
    repo: &git2::Repository,
) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if path.is_dir() {
            let mut sub_builder = repo.treebuilder(None)?;
            collect_files(&path, &mut sub_builder, repo)?;
            let sub_oid = sub_builder.write()?;
            tree_builder.insert(&*name_str, sub_oid, 0o040000)?;
        } else {
            let content = std::fs::read(&path)?;
            let oid = repo.blob(&content)?;
            tree_builder.insert(&*name_str, oid, 0o100644)?;
        }
    }
    Ok(())
}

// ── Graph Diff + Impact ─────────────────────────────────────────────────

fn cmd_diff(repo_path: &Path, commit: &str) -> anyhow::Result<()> {
    let canonical = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());

    // Get current graph from DuckDB
    let db = GraphDb::open(&canonical)?;
    if db.node_count()? == 0 {
        anyhow::bail!("No indexed graph. Run `cgx analyze` first.");
    }

    let all_nodes = db.get_all_nodes()?;
    let all_edges = db.get_all_edges()?;

    // Build the "after" snapshot from the indexed DB
    let after_nodes: Vec<cgx_engine::NodeDef> = all_nodes
        .iter()
        .map(|n| cgx_engine::NodeDef {
            id: n.id.clone(),
            kind: match n.kind.as_str() {
                "Function" => cgx_engine::NodeKind::Function,
                "Class" => cgx_engine::NodeKind::Class,
                "File" => cgx_engine::NodeKind::File,
                "Module" => cgx_engine::NodeKind::Module,
                "Variable" => cgx_engine::NodeKind::Variable,
                "Type" => cgx_engine::NodeKind::Type,
                "Author" => cgx_engine::NodeKind::Author,
                _ => cgx_engine::NodeKind::File,
            },
            name: n.name.clone(),
            path: n.path.clone(),
            line_start: n.line_start,
            line_end: n.line_end,
            ..Default::default()
        })
        .collect();

    let after_edges: Vec<cgx_engine::EdgeDef> = all_edges
        .iter()
        .map(|e| cgx_engine::EdgeDef {
            src: e.src.clone(),
            dst: e.dst.clone(),
            kind: match e.kind.as_str() {
                "CALLS" => cgx_engine::EdgeKind::Calls,
                "IMPORTS" => cgx_engine::EdgeKind::Imports,
                "INHERITS" => cgx_engine::EdgeKind::Inherits,
                "EXPORTS" => cgx_engine::EdgeKind::Exports,
                "CO_CHANGES" => cgx_engine::EdgeKind::CoChanges,
                "OWNS" => cgx_engine::EdgeKind::Owns,
                "DEPENDS_ON" => cgx_engine::EdgeKind::DependsOn,
                _ => cgx_engine::EdgeKind::Calls,
            },
            weight: e.weight,
            confidence: e.confidence,
        })
        .collect();

    let after = cgx_engine::GraphSnapshot {
        nodes: after_nodes,
        edges: after_edges,
        commit: "HEAD".to_string(),
    };

    // If diffing against HEAD, use the indexed graph as the before snapshot too
    let before = if commit == "HEAD" || commit == "head" {
        after.clone()
    } else {
        eprintln!("  Taking snapshot at {}...", commit);
        cgx_engine::snapshot_at_commit(&canonical, commit)?
    };

    let diff = cgx_engine::diff_graphs(&before, &after);

    println!();
    println!("  GRAPH DIFF: HEAD vs {}", commit);
    println!("  {}", "\u{2500}".repeat(50));
    println!(
        "  + Added:    {} nodes, {} edges",
        diff.added_nodes.len(),
        diff.added_edges.len()
    );
    println!(
        "  - Removed:  {} nodes, {} edges",
        diff.removed_nodes.len(),
        diff.removed_edges.len()
    );
    println!("  ~ Modified: {} nodes", diff.modified_nodes.len());

    if !diff.added_nodes.is_empty() {
        println!();
        println!("  NEW NODES:");
        for n in &diff.added_nodes {
            println!("    + {}:{} ", n.kind.as_str(), n.name);
        }
    }

    if !diff.removed_nodes.is_empty() {
        println!();
        println!("  REMOVED NODES:");
        for n in &diff.removed_nodes {
            println!("    - {}:{} ", n.kind.as_str(), n.name);
        }
    }

    if !diff.added_edges.is_empty() {
        println!();
        println!("  NEW EDGES (showing first 10):");
        for e in diff.added_edges.iter().take(10) {
            if let (Some(src_n), Some(dst_n)) = (
                before
                    .nodes
                    .iter()
                    .find(|n| n.id == e.src)
                    .or(after.nodes.iter().find(|n| n.id == e.src)),
                before
                    .nodes
                    .iter()
                    .find(|n| n.id == e.dst)
                    .or(after.nodes.iter().find(|n| n.id == e.dst)),
            ) {
                println!(
                    "    + {} → {} ({})",
                    src_n.name,
                    dst_n.name,
                    e.kind.as_str()
                );
            }
        }
    }

    if !diff.removed_edges.is_empty() {
        println!();
        println!("  REMOVED EDGES (showing first 10):");
        for e in diff.removed_edges.iter().take(10) {
            if let (Some(src_n), Some(dst_n)) = (
                before.nodes.iter().find(|n| n.id == e.src),
                after.nodes.iter().find(|n| n.id == e.dst),
            ) {
                println!(
                    "    - {} → {} ({})",
                    src_n.name,
                    dst_n.name,
                    e.kind.as_str()
                );
            }
        }
    }

    Ok(())
}

fn parse_duration_days(s: &str) -> anyhow::Result<u32> {
    let trimmed = s.trim();
    let num_part = trimmed
        .trim_end_matches('d')
        .trim_end_matches('D')
        .trim_end_matches("day")
        .trim_end_matches("days")
        .trim_end_matches("DAY")
        .trim_end_matches("DAYS");
    num_part.parse::<u32>().map_err(|_| {
        anyhow::anyhow!(
            "invalid duration: '{}'. Expected a number like '7' or '7d'",
            s
        )
    })
}

fn cmd_impact(repo_path: &Path, since_days: u32) -> anyhow::Result<()> {
    let canonical = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());

    eprintln!("  Analyzing changes in the last {} days...", since_days);
    let report = cgx_engine::compute_impact(&canonical, since_days)?;

    println!();
    println!("  IMPACT ANALYSIS — last {} days", since_days);
    println!("  {}", "\u{2500}".repeat(60));

    if report.changed_files.is_empty() {
        println!("  No changes detected in this period.");
        return Ok(());
    }

    println!("  Changed files: {}", report.changed_files.len());
    println!("  Directly changed nodes: {}", report.changed_nodes.len());
    println!("  Total impacted nodes (ripple): {}", report.total_impacted);

    if !report.changed_files.is_empty() {
        println!();
        println!("  CHANGED FILES:");
        for f in report.changed_files.iter().take(15) {
            let mut node_count = 0;
            for n in &report.changed_nodes {
                if &n.path == f {
                    node_count += 1;
                }
            }
            println!("    {} ({} nodes)", f, node_count);
        }
        if report.changed_files.len() > 15 {
            println!("    ... and {} more files", report.changed_files.len() - 15);
        }
    }

    if !report.impacted_nodes.is_empty() {
        println!();
        println!("  DOWNSTREAM IMPACT (things that depend on changed code):");
        for n in report.impacted_nodes.iter().take(15) {
            println!("    → {} ({})", n.name, n.kind);
        }
        if report.impacted_nodes.len() > 15 {
            println!(
                "    ... and {} more affected nodes",
                report.impacted_nodes.len() - 15
            );
        }

        let risk = if report.total_impacted > 50 {
            "CRITICAL"
        } else if report.total_impacted > 20 {
            "HIGH"
        } else if report.total_impacted > 5 {
            "MEDIUM"
        } else {
            "LOW"
        };
        println!();
        println!("  Risk level: {}", risk);
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// DOCTOR
// ─────────────────────────────────────────────────────────────────────────────

fn cmd_timeline(
    repo_path: &Path,
    commits: usize,
    since: Option<&str>,
    json: bool,
) -> anyhow::Result<()> {
    let db = GraphDb::open(repo_path)?;
    let entries = build_timeline(repo_path, &db, commits, since)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }

    println!();
    println!("  cgx timeline");
    println!("  {}", "\u{2500}".repeat(80));
    println!(
        "  {:<9}  {:<11}  {:>6}  {:>6}  {:>6}  Message",
        "SHA", "Date", "Files", "+ins", "-del"
    );
    println!("  {}", "\u{2500}".repeat(80));

    for entry in &entries {
        let sha_short = &entry.commit_sha[..8.min(entry.commit_sha.len())];
        let (file_count, insertions, deletions) = entry
            .snapshot_data
            .as_deref()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
            .map(|v| {
                let fc = v["file_count"].as_i64().unwrap_or(0);
                let ins = v["insertions"].as_i64().unwrap_or(0);
                let del = v["deletions"].as_i64().unwrap_or(0);
                (fc, ins, del)
            })
            .unwrap_or((0, 0, 0));

        let msg = if entry.commit_msg.len() > 48 {
            format!("{}...", &entry.commit_msg[..45])
        } else {
            entry.commit_msg.clone()
        };

        println!(
            "  {:<9}  {:<11}  {:>6}  {:>6}  {:>6}  {}",
            sha_short,
            entry.commit_date,
            file_count,
            format!("+{}", insertions),
            format!("-{}", deletions),
            msg
        );
    }

    println!("  {}", "\u{2500}".repeat(80));
    println!("  {} commit(s) shown", entries.len());
    println!("  Snapshots cached at: {}", db.db_path.display());
    println!();

    Ok(())
}

fn cmd_doctor() -> anyhow::Result<()> {
    use std::process::Command;

    println!();
    println!("  cgx doctor \u{2014} diagnostic report");
    println!("  {}", "\u{2500}".repeat(60));
    println!();

    let mut issues = 0usize;
    let mut warnings = 0usize;

    // 1. Binary info
    let exe = std::env::current_exe().ok();
    let version = env!("CARGO_PKG_VERSION");
    println!("  Binary");
    println!("    \u{2713} version {} ", version);
    if let Some(ref p) = exe {
        println!("    \u{2713} path: {}", p.display());
    } else {
        println!("    \u{2717} could not determine binary path");
        issues += 1;
    }
    println!();

    // 2. ~/.cgx directory
    let cgx_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cgx");
    if cgx_dir.exists() {
        let size = dir_size(&cgx_dir).unwrap_or(0);
        println!("  Data directory");
        println!("    \u{2713} {} exists", cgx_dir.display());
        println!("    \u{2713} size: {}", fmt_bytes(size));
    } else {
        println!("  Data directory");
        println!(
            "    \u{26A0} {} does not exist (will be created on first analyze)",
            cgx_dir.display()
        );
        warnings += 1;
    }
    println!();

    // 3. Registry
    match cgx_engine::Registry::load() {
        Ok(reg) => {
            println!("  Registry ({} repos indexed)", reg.repos.len());
            for entry in &reg.repos {
                let path_ok = entry.path.exists();
                let db_ok = entry.db_path.exists();
                let status = if path_ok && db_ok {
                    "\u{2713}"
                } else {
                    issues += 1;
                    "\u{2717}"
                };
                println!(
                    "    {} {}  ({} nodes, {} edges)  path:{}  db:{}",
                    status,
                    entry.name,
                    entry.node_count,
                    entry.edge_count,
                    if path_ok { "ok" } else { "MISSING" },
                    if db_ok { "ok" } else { "MISSING" }
                );
            }
        }
        Err(e) => {
            println!("  Registry");
            println!("    \u{2717} could not load registry: {}", e);
            issues += 1;
        }
    }
    println!();

    // 4. External tools
    println!("  External tools");
    let tools = vec![
        ("git", vec!["--version"]),
        ("node", vec!["--version"]),
        ("npm", vec!["--version"]),
    ];
    for (name, args) in tools {
        match Command::new(name).args(&args).output() {
            Ok(out) if out.status.success() => {
                let ver = String::from_utf8_lossy(&out.stdout).trim().to_string();
                println!("    \u{2713} {}  {}", name, ver);
            }
            _ => {
                if name == "git" {
                    println!("    \u{2717} {}  required for git history analysis", name);
                    issues += 1;
                } else {
                    println!(
                        "    \u{26A0} {}  needed for web UI builds and publish",
                        name
                    );
                    warnings += 1;
                }
            }
        }
    }
    println!();

    // 5. Editor integrations
    println!("  Editor integrations");
    let home = std::env::var("HOME").unwrap_or_default();
    let editors = vec![
        ("Claude Code", format!("{}/.claude/settings.json", home)),
        ("Cursor", format!("{}/.cursor/mcp.json", home)),
        ("VS Code", format!("{}/.vscode/settings.json", home)),
        ("Windsurf", format!("{}/.windsurf/mcp.json", home)),
        ("Zed", format!("{}/.config/zed/settings.json", home)),
    ];
    let mut any_editor = false;
    for (name, path) in editors {
        if Path::new(&path).exists() {
            any_editor = true;
            // Quick check if cgx is registered
            let registered = std::fs::read_to_string(&path)
                .ok()
                .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                .map(|v| v.to_string().contains("cgx"))
                .unwrap_or(false);
            if registered {
                println!("    \u{2713} {}  cgx registered", name);
            } else {
                println!(
                    "    \u{26A0} {}  detected but cgx not registered (run `cgx setup`)",
                    name
                );
                warnings += 1;
            }
        }
    }
    if !any_editor {
        println!("    \u{26A0} no supported editors detected (Claude Code, Cursor, VS Code, Windsurf, Zed)");
        warnings += 1;
    }
    println!();

    // 6. Skill files in current dir
    println!("  Skill files (current directory)");
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let skill_md = cwd.join("CGX_SKILL.md");
    let agents_md = cwd.join("AGENTS.md");
    if skill_md.exists() {
        println!("    \u{2713} CGX_SKILL.md present");
    } else {
        println!("    \u{26A0} CGX_SKILL.md missing (run `cgx analyze` to generate)");
        warnings += 1;
    }
    if agents_md.exists() {
        println!("    \u{2713} AGENTS.md present");
    } else {
        println!("    \u{26A0} AGENTS.md missing (run `cgx analyze` to generate)");
        warnings += 1;
    }
    println!();

    // 7. Git hooks
    println!("  Git hooks");
    let git_hooks_dir = cwd.join(".git").join("hooks");
    if git_hooks_dir.exists() {
        let post_commit = git_hooks_dir.join("post-commit");
        let post_merge = git_hooks_dir.join("post-merge");
        if post_commit.exists() {
            let content = std::fs::read_to_string(&post_commit).unwrap_or_default();
            if content.contains("cgx") {
                println!("    \u{2713} post-commit hook has cgx");
            } else {
                println!("    \u{26A0} post-commit hook present but does not mention cgx");
                warnings += 1;
            }
        } else {
            println!("    \u{26A0} post-commit hook missing");
            warnings += 1;
        }
        if post_merge.exists() {
            let content = std::fs::read_to_string(&post_merge).unwrap_or_default();
            if content.contains("cgx") {
                println!("    \u{2713} post-merge hook has cgx");
            } else {
                println!("    \u{26A0} post-merge hook present but does not mention cgx");
                warnings += 1;
            }
        } else {
            println!("    \u{26A0} post-merge hook missing");
            warnings += 1;
        }
    } else {
        println!("    \u{26A0} not a git repository (or no .git/hooks directory)");
        warnings += 1;
    }
    println!();

    // Summary
    println!("  {}", "\u{2500}".repeat(60));
    if issues == 0 && warnings == 0 {
        println!("  \u{2713} All checks passed. cgx is healthy.");
    } else {
        println!("  {} issue(s), {} warning(s) found.", issues, warnings);
        if issues > 0 {
            println!("  Run `cgx setup` to fix editor integrations.");
            println!("  Run `cgx analyze` to index the current repo and generate skill files.");
        }
    }
    println!();

    Ok(())
}

fn dir_size(path: &Path) -> anyhow::Result<u64> {
    let mut total = 0u64;
    for entry in walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            total += entry.metadata().map(|m| m.len()).unwrap_or(0);
        }
    }
    Ok(total)
}

fn fmt_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit = UNITS[0];
    for &u in &UNITS[1..] {
        if size < 1024.0 {
            break;
        }
        size /= 1024.0;
        unit = u;
    }
    format!("{:.1} {}", size, unit)
}

// ─────────────────────────────────────────────────────────────────────────────
// CLEAN
// ─────────────────────────────────────────────────────────────────────────────

fn cmd_clean(repo_path: &Path) -> anyhow::Result<()> {
    let canonical = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());
    let mut reg = cgx_engine::Registry::load()?;

    if let Some(pos) = reg
        .repos
        .iter()
        .position(|r| r.path.canonicalize().ok().as_ref() == Some(&canonical))
    {
        let entry = reg.repos.remove(pos);
        if entry.db_path.exists() {
            std::fs::remove_file(&entry.db_path)?;
            println!("  \u{2713} removed database: {}", entry.db_path.display());
        }
        reg.save()?;
        println!("  \u{2713} removed registry entry for: {}", entry.name);
    } else {
        println!(
            "  \u{26A0} no indexed repo found at: {}",
            canonical.display()
        );
    }

    // Optionally remove skill files if they exist in the repo root
    let skill_md = canonical.join("CGX_SKILL.md");
    let agents_md = canonical.join("AGENTS.md");
    if skill_md.exists() {
        println!("  hint: CGX_SKILL.md still exists in repo root (remove manually if desired)");
    }
    if agents_md.exists() {
        println!("  hint: AGENTS.md still exists in repo root (remove manually if desired)");
    }

    Ok(())
}

fn cmd_clean_all() -> anyhow::Result<()> {
    let mut reg = cgx_engine::Registry::load()?;
    let count = reg.repos.len();

    for entry in reg.repos.drain(..) {
        if entry.db_path.exists() {
            let _ = std::fs::remove_file(&entry.db_path);
        }
    }
    reg.save()?;

    println!("  \u{2713} removed {} indexed repositories", count);
    println!("  \u{2713} all DuckDB graph databases deleted");

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// UPDATE
// ─────────────────────────────────────────────────────────────────────────────

const UPDATE_CHECK_INTERVAL_SECS: i64 = 60 * 60 * 24;
const LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/AayushBahukhandi/cgx/releases/latest";

#[derive(serde::Deserialize, serde::Serialize)]
struct UpdateCheckCache {
    latest_version: String,
    checked_at: i64,
}

fn maybe_show_update_notice() {
    if update_check_disabled() {
        return;
    }

    let current = env!("CARGO_PKG_VERSION");
    let cache_path = update_check_cache_path();
    let cached = read_update_check_cache(&cache_path);

    if let Some(cache) = cached.as_ref() {
        if version_is_newer(&cache.latest_version, current) {
            print_update_notice(current, &cache.latest_version);
            return;
        }

        let now = chrono::Utc::now().timestamp();
        if now.saturating_sub(cache.checked_at) < UPDATE_CHECK_INTERVAL_SECS {
            return;
        }
    }

    let latest = match fetch_latest_version_blocking() {
        Ok(version) => version,
        Err(_) => return,
    };

    let _ = write_update_check_cache(
        &cache_path,
        &UpdateCheckCache {
            latest_version: latest.clone(),
            checked_at: chrono::Utc::now().timestamp(),
        },
    );

    if version_is_newer(&latest, current) {
        print_update_notice(current, &latest);
    }
}

fn update_check_disabled() -> bool {
    std::env::var("CGX_NO_UPDATE_CHECK")
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

fn update_check_cache_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cgx")
        .join("update-check.json")
}

fn read_update_check_cache(path: &Path) -> Option<UpdateCheckCache> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
}

fn write_update_check_cache(path: &Path, cache: &UpdateCheckCache) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_vec(cache)?)?;
    Ok(())
}

fn fetch_latest_version_blocking() -> anyhow::Result<String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(async {
        let client = reqwest::Client::builder()
            .user_agent("cgx-cli")
            .timeout(Duration::from_millis(900))
            .build()?;

        let resp = client.get(LATEST_RELEASE_URL).send().await?;
        if !resp.status().is_success() {
            anyhow::bail!("GitHub release check failed: {}", resp.status());
        }

        let body: serde_json::Value = resp.json().await?;
        let tag = body["tag_name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("GitHub release response missing tag_name"))?;

        Ok(tag.trim_start_matches('v').to_string())
    })
}

fn version_is_newer(candidate: &str, current: &str) -> bool {
    let candidate_parts = parse_version_parts(candidate);
    let current_parts = parse_version_parts(current);

    for idx in 0..candidate_parts.len().max(current_parts.len()) {
        let candidate_part = *candidate_parts.get(idx).unwrap_or(&0);
        let current_part = *current_parts.get(idx).unwrap_or(&0);
        if candidate_part > current_part {
            return true;
        }
        if candidate_part < current_part {
            return false;
        }
    }

    false
}

fn parse_version_parts(version: &str) -> Vec<u64> {
    version
        .split(['.', '-', '+'])
        .take_while(|part| part.chars().all(|c| c.is_ascii_digit()))
        .filter_map(|part| part.parse().ok())
        .collect()
}

fn print_update_notice(current: &str, latest: &str) {
    eprintln!();
    eprintln!(
        "  Update available: {} → {}",
        current,
        latest.trim_start_matches('v')
    );
    eprintln!("  Run `cgx update --auto` to upgrade automatically, or:");
    eprintln!("    brew upgrade aayushbahukhandi/cgx/cgx");
    eprintln!("    cargo install cgx-cli");
    eprintln!("  Release notes: https://github.com/AayushBahukhandi/cgx/releases/latest");
    eprintln!();
}

fn cmd_update(auto: bool) -> anyhow::Result<()> {
    use std::process::Command;

    let current = env!("CARGO_PKG_VERSION");
    println!();
    println!("  cgx update");
    println!("  {}", "\u{2500}".repeat(60));
    println!("  installed version: {}", current);
    match fetch_latest_version_blocking() {
        Ok(latest) => {
            let _ = write_update_check_cache(
                &update_check_cache_path(),
                &UpdateCheckCache {
                    latest_version: latest.clone(),
                    checked_at: chrono::Utc::now().timestamp(),
                },
            );
            println!("  latest version:    {}", latest);
            if version_is_newer(&latest, current) {
                println!("  status:            update available");
            } else {
                println!("  status:            up to date");
            }
        }
        Err(_) => {
            println!("  latest version:    could not check");
        }
    }
    println!();

    // Try to detect installation method
    let exe = std::env::current_exe().ok();
    let is_cargo = exe
        .as_ref()
        .map(|p| p.to_string_lossy().contains(".cargo") || p.to_string_lossy().contains("cargo"))
        .unwrap_or(false);
    let is_homebrew = exe
        .as_ref()
        .map(|p| p.to_string_lossy().contains("Cellar") || p.to_string_lossy().contains("homebrew"))
        .unwrap_or(false);

    if auto {
        if is_cargo {
            println!("  Detected cargo installation. Running: cargo install cgx-cli");
            let status = Command::new("cargo")
                .args(["install", "cgx-cli"])
                .status()?;
            if status.success() {
                println!("  \u{2713} update complete");
            } else {
                anyhow::bail!("cargo install failed");
            }
        } else if is_homebrew {
            println!(
                "  Detected Homebrew installation. Running: brew upgrade aayushbahukhandi/cgx/cgx"
            );
            let status = Command::new("brew")
                .args(["upgrade", "aayushbahukhandi/cgx/cgx"])
                .status()?;
            if status.success() {
                println!("  \u{2713} update complete — restart your shell or open a new terminal");
            } else {
                anyhow::bail!("brew upgrade failed");
            }
        } else {
            println!("  Could not detect installation method.");
            println!("  Please download the latest binary from:");
            println!("  https://github.com/AayushBahukhandi/cgx/releases/latest");
        }
    } else {
        println!("  How to update cgx depends on how you installed it:");
        println!();
        println!("  cargo install:");
        println!("    cargo install cgx-cli");
        println!();
        println!("  Homebrew:");
        println!("    brew upgrade aayushbahukhandi/cgx/cgx");
        println!();
        println!("  Pre-built binary:");
        println!("    Download latest release from:");
        println!("    https://github.com/AayushBahukhandi/cgx/releases/latest");
        println!();
        println!("  Or run with --auto to attempt automatic update:");
        println!("    cgx update --auto");
        println!();
    }

    Ok(())
}
