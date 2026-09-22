use std::path::{Path, PathBuf};
use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;

mod installer;
mod hooks;

use codeatlas_core::Engine;

#[derive(Parser)]
#[command(
    name = "codeatlas",
    version,
    about = "Ultra-fast code intelligence and knowledge graph engine in Rust",
    long_about = "CodeAtlas provides instant codebase indexing, multi-signal hybrid code retrieval, living wiki generation with Mermaid diagrams, and deep impact analysis."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Index a codebase directory into SQLite & Graph
    Index {
        /// Root path of the codebase to index
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Force re-indexing all files, ignoring incremental manifest cache
        #[arg(long)]
        full: bool,

        /// Enable ultra-fast indexing mode (metadata-only cache check & in-memory WAL write buffer)
        #[arg(short = 'f', long)]
        fast: bool,

        /// Skip generating semantic vector embeddings
        #[arg(long)]
        no_embeddings: bool,

        /// Embedding approach: 'model2vec' (default, pure Rust, no ORT) or 'ort' (ONNX Runtime transformer)
        #[arg(long, default_value = "model2vec")]
        embedder: String,

        /// Custom path to the SQLite index database
        #[arg(long)]
        db: Option<PathBuf>,
    },

    /// Search indexed code using hybrid multi-signal retrieval
    Search {
        /// Natural language or keyword query
        query: String,

        /// Maximum number of results to return
        #[arg(short = 'n', long, default_value = "10")]
        limit: usize,

        /// Expand results with 1-hop graph neighborhood
        #[arg(long)]
        expand_graph: bool,

        /// Fast search mode: direct sub-millisecond lexical scoring without graph expansion
        #[arg(long)]
        fast: bool,

        /// Disable semantic search (lexical BM25 only)
        #[arg(long)]
        no_semantic: bool,

        /// Embedding approach for query: 'model2vec' (default) or 'ort'
        #[arg(long, default_value = "model2vec")]
        embedder: String,

        /// Output results as JSON
        #[arg(long)]
        json: bool,

        /// Custom path to the SQLite index database
        #[arg(long)]
        db: Option<PathBuf>,
    },

    /// Reverse impact analysis: find callers and dependents of a symbol
    Impact {
        /// Target symbol name or node ID
        target: String,

        /// Output results as JSON
        #[arg(long)]
        json: bool,

        /// Custom path to the SQLite index database
        #[arg(long)]
        db: Option<PathBuf>,
    },

    /// Generate living architecture documentation with Mermaid diagrams
    Wiki {
        /// Destination directory for generated wiki
        #[arg(default_value = ".codeatlas/wiki")]
        output: PathBuf,

        /// Custom path to the SQLite index database
        #[arg(long)]
        db: Option<PathBuf>,
    },

    /// Watch repository for file modifications and incrementally re-index
    Watch {
        /// Root path of the codebase to watch
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Polling interval in seconds
        #[arg(short = 'i', long, default_value = "3")]
        interval: u64,

        /// Custom path to the SQLite index database
        #[arg(long)]
        db: Option<PathBuf>,
    },

    /// Inspect or export the knowledge graph
    Graph {
        #[command(subcommand)]
        sub: GraphSubcommands,
    },

    /// Auto-configure CodeAtlas MCP in coding agents (claude, cursor, opencode, codex)
    Install {
        /// Target agent: claude, cursor, opencode, codex, or all
        #[arg(default_value = "all")]
        agent: String,
    },

    /// Remove CodeAtlas MCP from coding agents
    Uninstall {
        /// Target agent: claude, cursor, opencode, codex, or all
        #[arg(default_value = "all")]
        agent: String,
    },

    /// Manage Git hooks for automatic background indexing
    Hook {
        #[command(subcommand)]
        sub: HookSubcommands,
    },
}

#[derive(Subcommand)]
enum HookSubcommands {
    /// Install post-commit, post-checkout, and post-merge git hooks
    Install {
        /// Repository path
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Remove CodeAtlas git hooks
    Uninstall {
        /// Repository path
        #[arg(default_value = ".")]
        path: PathBuf,
    },
}

#[derive(Subcommand)]
enum GraphSubcommands {
    /// Display graph and index statistics
    Stats {
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Inspect symbol details and its 1-hop callers and callees
    Symbol {
        /// Symbol name or ID
        name: String,

        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Export graph to DOT or JSON format
    Export {
        /// Format: json or dot
        #[arg(short, long, default_value = "json")]
        format: String,

        /// Output file path (defaults to stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,

        #[arg(long)]
        db: Option<PathBuf>,
    },
}

fn resolve_db_path(root: &Path, db: Option<PathBuf>) -> PathBuf {
    if let Some(d) = db {
        d
    } else {
        root.join(".codeatlas").join("index.db")
    }
}

fn parse_embedder_kind(s: &str) -> codeatlas_core::embedder::EmbedderKind {
    match s.to_lowercase().as_str() {
        "ort" | "onnx" | "fastembed" | "bge" => codeatlas_core::embedder::EmbedderKind::Ort,
        _ => codeatlas_core::embedder::EmbedderKind::Model2Vec,
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Index { path, full, fast, no_embeddings, embedder, db } => {
            let root = std::fs::canonicalize(&path).unwrap_or(path);
            let db_path = resolve_db_path(&root, db);
            let kind = parse_embedder_kind(&embedder);

            let mode_str = if fast {
                " [Fast Mode]".yellow().to_string()
            } else if no_embeddings {
                " [No Embeddings]".dimmed().to_string()
            } else {
                format!(" [Hybrid Semantic - {}]", kind).magenta().to_string()
            };
            println!("⚡ {}{}", "CodeAtlas (Rust)".bold().cyan(), mode_str);
            println!("  Indexing directory: {}", root.display().to_string().yellow());
            println!("  Database target:    {}", db_path.display().to_string().dimmed());

            let mut engine = Engine::with_embedder_kind(&root, &db_path, kind)?;
            let report = engine.index_with_options(full, fast || no_embeddings)?;

            println!("\n{}", "✓ Indexing Complete".bold().green());
            println!("  Files scanned:   {}", report.files_scanned.to_string().bold());
            println!("  Files indexed:   {}", report.files_indexed.to_string().bold());
            println!("  Symbols parsed:  {}", report.symbols_count.to_string().bold());
            println!("  Chunks indexed:  {}", report.chunks_count.to_string().bold());
            println!("  Graph edges:     {}", report.edges_count.to_string().bold());
            let emb_count = engine.db.get_embedding_count().unwrap_or(0);
            if emb_count > 0 {
                println!("  Vectors embedded:{}", emb_count.to_string().bold().magenta());
            }
            println!("  Total latency:   {} ms", report.duration_ms.to_string().cyan().bold());
        }

        Commands::Search {
            query,
            limit,
            expand_graph,
            fast,
            no_semantic,
            embedder,
            json,
            db,
        } => {
            let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let db_path = resolve_db_path(&root, db);
            let kind = parse_embedder_kind(&embedder);

            if !db_path.exists() {
                eprintln!(
                    "{} Index database not found at {}. Run `codeatlas index` first.",
                    "Error:".bold().red(),
                    db_path.display()
                );
                std::process::exit(1);
            }

            let engine = Engine::with_embedder_kind(&root, &db_path, kind)?;
            let results = engine.search_with_options(&query, limit, expand_graph, fast || no_semantic)?;

            if json {
                println!("{}", serde_json::to_string_pretty(&results)?);
                return Ok(());
            }

            println!(
                "{} for \"{}\" ({} results):\n",
                "CodeAtlas Search".bold().cyan(),
                query.yellow(),
                results.len()
            );

            for (idx, r) in results.iter().enumerate() {
                let sym_str = r.symbol_id.as_deref().unwrap_or("file");
                let def_badge = if r.is_definition {
                    "[definition]".green()
                } else {
                    "".normal()
                };
                let rank_info = match (r.lexical_rank, r.semantic_rank) {
                    (Some(l), Some(s)) => format!(" [lex:#{} sem:#{}]", l, s).magenta(),
                    (Some(l), None) => format!(" [lex:#{}]", l).dimmed(),
                    (None, Some(s)) => format!(" [sem:#{}]", s).magenta(),
                    (None, None) => "".normal(),
                };

                println!(
                    "{}. {} {}:{} (score: {:.4}){} {}",
                    (idx + 1).to_string().bold(),
                    r.file_path.bold().blue(),
                    r.start_line,
                    r.end_line,
                    r.score,
                    rank_info,
                    def_badge
                );
                println!("   Symbol: {}", sym_str.dimmed());

                let preview_lines: Vec<&str> = r.content.lines().take(4).collect();
                for l in preview_lines {
                    println!("   │ {}", l.dimmed());
                }

                if !r.neighbors.is_empty() {
                    let neighbor_names: Vec<String> = r
                        .neighbors
                        .iter()
                        .take(5)
                        .map(|n| format!("{} ({})", n.name, n.kind))
                        .collect();
                    println!("   └ 1-hop: {}", neighbor_names.join(", ").magenta());
                }
                println!();
            }
        }

        Commands::Impact { target, json, db } => {
            let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let db_path = resolve_db_path(&root, db);

            if !db_path.exists() {
                eprintln!("{} Index database not found at {}.", "Error:".bold().red(), db_path.display());
                std::process::exit(1);
            }

            let engine = Engine::open(&root, &db_path)?;
            let affected = engine.impact(&target);

            if json {
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                    "target": target,
                    "affected_count": affected.len(),
                    "affected": affected,
                }))?);
                return Ok(());
            }

            println!(
                "{} for \"{}\" ({} callers/dependents):\n",
                "Reverse Impact Analysis".bold().cyan(),
                target.yellow(),
                affected.len()
            );

            if affected.is_empty() {
                println!("  No dependent callers detected in current knowledge graph.");
            } else {
                for node in &affected {
                    println!("  ← {}", node.red());
                }
            }
        }

        Commands::Wiki { output, db } => {
            let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let db_path = resolve_db_path(&root, db);

            if !db_path.exists() {
                eprintln!("{} Index database not found at {}.", "Error:".bold().red(), db_path.display());
                std::process::exit(1);
            }

            let engine = Engine::open(&root, &db_path)?;
            let files = engine.generate_wiki(&output)?;

            println!("{}", "✓ Living Architecture Wiki Generated".bold().green());
            println!("  Output directory: {}", output.display().to_string().cyan());
            println!("  Chapters created: {}", files.len().to_string().bold());
            for f in &files {
                let rel = f.strip_prefix(&output).unwrap_or(f);
                println!("  - {}", rel.display());
            }
        }

        Commands::Watch { path, interval, db } => {
            let root = std::fs::canonicalize(&path).unwrap_or(path);
            let db_path = resolve_db_path(&root, db);

            println!("⚡ {} [Watch Mode]", "CodeAtlas (Rust)".bold().cyan());
            println!("  Watching directory: {}", root.display().to_string().yellow());
            println!("  Polling interval:   {}s (Press Ctrl+C to stop)\n", interval);

            let mut engine = Engine::open(&root, &db_path)?;
            let initial_report = engine.index_with_options(false, true)?;
            let now = chrono_now_time();
            println!(
                "  [{}] Initial index: {} files indexed, {} symbols, {} ms",
                now.dimmed(),
                initial_report.files_indexed,
                initial_report.symbols_count,
                initial_report.duration_ms
            );

            loop {
                std::thread::sleep(std::time::Duration::from_secs(interval));
                match engine.index_with_options(false, true) {
                    Ok(report) => {
                        if report.files_indexed > 0 {
                            let now = chrono_now_time();
                            println!(
                                "  [{}] {} Re-indexed {} file(s) in {} ms (total {} symbols)",
                                now.dimmed(),
                                "✓".green().bold(),
                                report.files_indexed,
                                report.duration_ms,
                                report.symbols_count
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("  {} Index error: {}", "Warning:".yellow().bold(), e);
                    }
                }
            }
        }

        Commands::Graph { sub } => match sub {
            GraphSubcommands::Stats { db } => {
                let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                let db_path = resolve_db_path(&root, db);

                if !db_path.exists() {
                    eprintln!("{} Index database not found at {}.", "Error:".bold().red(), db_path.display());
                    std::process::exit(1);
                }

                let engine = Engine::open(&root, &db_path)?;
                let stats = engine.db.get_stats()?;
                println!("{}", "CodeAtlas Knowledge Graph Stats:".bold().cyan());
                println!("{}", serde_json::to_string_pretty(&stats)?);
            }
            GraphSubcommands::Symbol { name, db } => {
                let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                let db_path = resolve_db_path(&root, db);

                if !db_path.exists() {
                    eprintln!("{} Index database not found at {}.", "Error:".bold().red(), db_path.display());
                    std::process::exit(1);
                }

                let engine = Engine::open(&root, &db_path)?;
                let symbols = engine.get_symbols()?;
                let matches: Vec<_> = symbols
                    .iter()
                    .filter(|s| s.name == name || s.node_id.ends_with(&format!(":{}", name)) || s.node_id.contains(&name))
                    .collect();

                if matches.is_empty() {
                    println!("{} No symbol found matching '{}'.", "Notice:".yellow(), name);
                } else {
                    println!("{} for '{}' ({} matches):\n", "Symbol Inspection".bold().cyan(), name.yellow(), matches.len());
                    for s in matches {
                        println!("• {} ({})", s.name.bold(), s.kind.as_str().cyan());
                        println!("  Node ID:    {}", s.node_id.dimmed());
                        println!("  File:       {}:{}-{}", s.file_path.blue(), s.start_line, s.end_line);
                        if let Some(ref sig) = s.signature {
                            println!("  Signature:  {}", sig.dimmed());
                        }
                        if let Some(ref doc) = s.docstring {
                            println!("  Docstring:  {}", doc.lines().next().unwrap_or("").dimmed());
                        }
                        let neighbors = engine.graph.expand_neighborhood(std::slice::from_ref(&s.node_id), 1);
                        let clean: Vec<_> = neighbors.iter().filter(|&n| n != &s.node_id).take(6).collect();
                        if !clean.is_empty() {
                            println!("  1-Hop Neighbors: {}", clean.iter().map(|n| n.as_str()).collect::<Vec<_>>().join(", ").magenta());
                        }
                        let callers = engine.impact(&s.node_id);
                        if !callers.is_empty() {
                            println!("  Incoming Callers/Dependents ({}): {}", callers.len(), callers.iter().take(5).cloned().collect::<Vec<_>>().join(", ").red());
                        }
                        println!();
                    }
                }
            }

            GraphSubcommands::Export { format, output, db } => {
                let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                let db_path = resolve_db_path(&root, db);

                if !db_path.exists() {
                    eprintln!("{} Index database not found at {}.", "Error:".bold().red(), db_path.display());
                    std::process::exit(1);
                }

                let engine = Engine::open(&root, &db_path)?;
                let content = match format.to_lowercase().as_str() {
                    "dot" => engine.export_graph_dot(),
                    _ => serde_json::to_string_pretty(&engine.export_graph_json())?,
                };

                if let Some(out_path) = output {
                    std::fs::write(&out_path, &content)?;
                    println!("✓ Exported graph to {}", out_path.display().to_string().green());
                } else {
                    println!("{}", content);
                }
            }
        },

        Commands::Install { agent } => {
            let targets = if agent == "all" {
                vec!["claude", "cursor", "opencode", "codex"]
            } else {
                vec![agent.as_str()]
            };

            println!("{}", "Configuring CodeAtlas MCP for coding agents:".bold().cyan());
            for target in targets {
                let res = installer::configure_agent(target, false)?;
                installer::print_install_results(&res);
            }
        }

        Commands::Uninstall { agent } => {
            let targets = if agent == "all" {
                vec!["claude", "cursor", "opencode", "codex"]
            } else {
                vec![agent.as_str()]
            };

            println!("{}", "Removing CodeAtlas MCP from coding agents:".bold().yellow());
            for target in targets {
                let res = installer::configure_agent(target, true)?;
                installer::print_install_results(&res);
            }
        }

        Commands::Hook { sub } => match sub {
            HookSubcommands::Install { path } => {
                let root = std::fs::canonicalize(&path).unwrap_or(path);
                match hooks::install_git_hooks(&root) {
                    Ok(installed) => {
                        println!("{} Installed CodeAtlas git hooks: {}", "✓".green().bold(), installed.join(", ").cyan());
                        println!("  CodeAtlas will now automatically re-index incrementally in the background on git commits and checkouts.");
                    }
                    Err(e) => {
                        eprintln!("{} Failed to install git hooks: {}", "Error:".red().bold(), e);
                        std::process::exit(1);
                    }
                }
            }
            HookSubcommands::Uninstall { path } => {
                let root = std::fs::canonicalize(&path).unwrap_or(path);
                match hooks::uninstall_git_hooks(&root) {
                    Ok(uninstalled) => {
                        if uninstalled.is_empty() {
                            println!("No CodeAtlas git hooks were found in repository.");
                        } else {
                            println!("{} Removed CodeAtlas git hooks: {}", "✓".yellow().bold(), uninstalled.join(", ").yellow());
                        }
                    }
                    Err(e) => {
                        eprintln!("{} Failed to remove git hooks: {}", "Error:".red().bold(), e);
                        std::process::exit(1);
                    }
                }
            }
        },
    }

    Ok(())
}

fn chrono_now_time() -> String {
    let now = std::time::SystemTime::now();
    let d = now.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let secs = d.as_secs();
    let hours = (secs / 3600) % 24;
    let mins = (secs / 60) % 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}", hours, mins, s)
}

