use std::path::{Path, PathBuf};
use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;

mod installer;

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
}

#[derive(Subcommand)]
enum GraphSubcommands {
    /// Display graph and index statistics
    Stats {
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Index { path, full, db } => {
            let root = std::fs::canonicalize(&path).unwrap_or(path);
            let db_path = resolve_db_path(&root, db);

            println!("{}", "⚡ CodeAtlas (Rust)".bold().cyan());
            println!("  Indexing directory: {}", root.display().to_string().yellow());
            println!("  Database target:    {}", db_path.display().to_string().dimmed());

            let mut engine = Engine::open(&root, &db_path)?;
            let report = engine.index(full)?;

            println!("\n{}", "✓ Indexing Complete".bold().green());
            println!("  Files scanned:   {}", report.files_scanned.to_string().bold());
            println!("  Files indexed:   {}", report.files_indexed.to_string().bold());
            println!("  Symbols parsed:  {}", report.symbols_count.to_string().bold());
            println!("  Chunks indexed:  {}", report.chunks_count.to_string().bold());
            println!("  Graph edges:     {}", report.edges_count.to_string().bold());
            println!("  Total latency:   {} ms", report.duration_ms.to_string().cyan().bold());
        }

        Commands::Search {
            query,
            limit,
            expand_graph,
            json,
            db,
        } => {
            let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let db_path = resolve_db_path(&root, db);

            if !db_path.exists() {
                eprintln!(
                    "{} Index database not found at {}. Run `codeatlas index` first.",
                    "Error:".bold().red(),
                    db_path.display()
                );
                std::process::exit(1);
            }

            let engine = Engine::open(&root, &db_path)?;
            let results = engine.search(&query, limit, expand_graph)?;

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

                println!(
                    "{}. {} {}:{} (score: {:.4}) {}",
                    (idx + 1).to_string().bold(),
                    r.file_path.bold().blue(),
                    r.start_line,
                    r.end_line,
                    r.score,
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
    }

    Ok(())
}
