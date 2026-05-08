mod chat;
mod tui;

use clap::{Parser, Subcommand};
use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use cgx_engine::{
    analyze_repo, export_dot, export_graphml, export_json, export_mermaid, export_svg, resolve,
    run_clustering, walk_repo, Edge, EdgeKind, GraphDb, Node, NodeKind, ParserRegistry, Registry,
    RepoEntry, TagRow,
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

        /// Filter by annotation type (TODO, FIXME, HACK, NOTE, BUG, OPTIMIZE, WARN, XXX)
        #[arg(long)]
        tag: Option<String>,

        /// Filter by comment source: code, jsx, or jsx_commented_code
        #[arg(long)]
        kind: Option<String>,

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
    /// Find unreferenced exports
    DeadCode {
        #[arg(long)]
        repo: Option<PathBuf>,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if !matches!(&cli.command, Commands::Update { .. }) {
        maybe_show_update_notice();
    }

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
            QueryCmd::DeadCode { repo } => cmd_query_dead_code(repo),
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
            tag,
            kind,
            json,
        } => {
            let repo_path = repo.unwrap_or_else(|| PathBuf::from("."));
            cmd_todos(&repo_path, tag.as_deref(), kind.as_deref(), json)
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
    let entry = format!(
        "\n# cgx\n- **cgx** (`~/.claude/skills/cgx/SKILL.md`) - index any Git repo as a queryable knowledge graph. Trigger: `/cgx`\nWhen the user types `/cgx`, invoke the Skill tool with `skill: \"cgx\"` before doing anything else.\n"
    );
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

fn cmd_query_dead_code(repo: Option<PathBuf>) -> anyhow::Result<()> {
    let db = GraphDb::open(&resolve_repo(repo))?;
    let all = db.get_all_nodes()?;
    let dead: Vec<_> = all
        .iter()
        .filter(|n| n.in_degree == 0 && n.kind != "File" && n.kind != "Author")
        .collect();
    if dead.is_empty() {
        println!("No dead code detected.");
    } else {
        println!("  Potentially unused symbols (nothing references them):");
        for n in dead.iter().take(30) {
            println!("    {}  {:<20}  {}", n.kind, n.name, n.path);
        }
    }
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
        println!("  No annotation comments found. Run `cgx analyze` to index the codebase.");
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
    collect_files(dist_dir, dist_dir, &mut tree_builder, &repo)?;
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
    base: &Path,
    dir: &Path,
    tree_builder: &mut git2::TreeBuilder,
    repo: &git2::Repository,
) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            collect_files(base, &path, tree_builder, repo)?;
        } else {
            let content = std::fs::read(&path)?;
            let oid = repo.blob(&content)?;
            let rel_path = path
                .strip_prefix(base)?
                .to_string_lossy()
                .replace('\\', "/");
            tree_builder.insert(&rel_path, oid, 0o100644)?;
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
        "  Update available: {} -> {}",
        current,
        latest.trim_start_matches('v')
    );
    eprintln!("  Run brew upgrade aayushbahukhandi/cgx/cgx or cargo install cgx-cli to update.");
    eprintln!("  See full release notes:");
    eprintln!("  https://github.com/AayushBahukhandi/cgx/releases/latest");
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
