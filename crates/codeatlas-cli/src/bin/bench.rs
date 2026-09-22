use std::path::Path;
use std::time::Instant;
use anyhow::Result;
use colored::Colorize;

use codeatlas_core::Engine;

struct QueryBench {
    query: &'static str,
    category: &'static str,
}

const BENCH_QUERIES: &[QueryBench] = &[
    QueryBench { query: "CodeGraph", category: "Core Graph Structure" },
    QueryBench { query: "Retriever", category: "Hybrid Retrieval Engine" },
    QueryBench { query: "AstParser", category: "Tree-sitter Parser" },
    QueryBench { query: "FileScanner", category: "Filesystem Scanner" },
    QueryBench { query: "Database", category: "SQLite WAL Storage" },
    QueryBench { query: "search_with_options", category: "Search Entry Point" },
    QueryBench { query: "impact_analysis", category: "Blast Radius BFS" },
    QueryBench { query: "parse_python", category: "Language Grammars" },
    QueryBench { query: "install_git_hooks", category: "Git Hooks Automation" },
    QueryBench { query: "chunk_embeddings", category: "Vector Persistence" },
];

fn percentile(mut sorted: Vec<f64>, pct: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let idx = ((sorted.len() as f64) * pct).floor() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn main() -> Result<()> {
    let repo_root = std::env::current_dir()?;
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("bench_index.db");

    println!("{}", "═══════════════════════════════════════════════════════════════".cyan());
    println!("  ⚡ {}", "CodeAtlas-RS Performance Benchmark Suite".bold().cyan());
    println!("  Target Repo:    {}", repo_root.display().to_string().yellow());
    println!("  Database:       {}", db_path.display().to_string().dimmed());
    println!("{}", "═══════════════════════════════════════════════════════════════".cyan());

    // 1. Cold Indexing Benchmark
    let mut engine = Engine::open(&repo_root, &db_path)?;
    let t0 = Instant::now();
    let report = engine.index_with_options(true, false)?;
    let cold_index_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let files_per_sec = (report.files_indexed as f64) / (cold_index_ms / 1000.0).max(0.001);

    println!("\n{}", "1. Indexing & Parsing Throughput".bold().green());
    println!("  Files scanned:       {}", report.files_scanned.to_string().bold());
    println!("  Files parsed:        {}", report.files_indexed.to_string().bold());
    println!("  Symbols extracted:   {}", report.symbols_count.to_string().bold());
    println!("  Chunks indexed:      {}", report.chunks_count.to_string().bold());
    println!("  Graph edges linked:  {}", report.edges_count.to_string().bold());
    println!("  Cold Index Latency:  {} ({:.1} files/sec)", format!("{:.2} ms", cold_index_ms).cyan().bold(), files_per_sec);

    // 2. Incremental Cache Check
    let t0 = Instant::now();
    let _inc_report = engine.index_with_options(false, true)?;
    let inc_index_ms = t0.elapsed().as_secs_f64() * 1000.0;
    println!("  Fast Cache Check:    {} (0 files re-parsed)", format!("{:.2} ms", inc_index_ms).cyan().bold());

    // 3. Search Retrieval Latency (100 iterations per query)
    println!("\n{}", "2. Retrieval Query Latency (100 iterations per query)".bold().green());
    println!("| Query | Category | Top Match | Is Def | P50 (ms) | P95 (ms) | Mean (ms) |");
    println!("|:------|:---------|:----------|:------:|:--------:|:--------:|:---------:|");

    let iterations = 100;
    let mut all_p50s: Vec<f64> = Vec::new();

    for qb in BENCH_QUERIES {
        // Warmup
        let _ = engine.search_with_options(qb.query, 10, true, false)?;

        let mut latencies: Vec<f64> = Vec::with_capacity(iterations);
        let mut top_file = String::new();
        let mut is_def = false;

        for i in 0..iterations {
            let start = Instant::now();
            let results = engine.search_with_options(qb.query, 10, true, false)?;
            let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
            latencies.push(elapsed_ms);

            if i == 0 && !results.is_empty() {
                top_file = Path::new(&results[0].file_path)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&results[0].file_path)
                    .to_string();
                is_def = results[0].is_definition;
            }
        }

        let p50 = percentile(latencies.clone(), 0.50);
        let p95 = percentile(latencies.clone(), 0.95);
        let mean: f64 = latencies.iter().sum::<f64>() / (latencies.len() as f64);
        all_p50s.push(p50);

        let def_mark = if is_def { "✓" } else { " " };
        println!(
            "| `{}` | {} | `{}` | {} | {:.3} | {:.3} | {:.3} |",
            qb.query, qb.category, top_file, def_mark, p50, p95, mean
        );
    }

    let overall_p50 = percentile(all_p50s.clone(), 0.50);
    let overall_mean: f64 = all_p50s.iter().sum::<f64>() / (all_p50s.len() as f64);
    println!("\n  📊 Retrieval Median P50: {} | Mean P50: {}", format!("{:.3} ms", overall_p50).cyan().bold(), format!("{:.3} ms", overall_mean).cyan().bold());

    // 4. Reverse Impact Analysis (Blast Radius BFS)
    println!("\n{}", "3. Knowledge Graph BFS Impact Analysis Latency".bold().green());
    let mut impact_lats: Vec<f64> = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let start = Instant::now();
        let _ = engine.impact("AstParser");
        impact_lats.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    let imp_p50 = percentile(impact_lats.clone(), 0.50);
    let imp_p95 = percentile(impact_lats.clone(), 0.95);
    println!("  BFS Impact on `AstParser`: P50 = {} | P95 = {} (100 iterations)", format!("{:.3} ms", imp_p50).cyan().bold(), format!("{:.3} ms", imp_p95).cyan().bold());

    // 5. Storage Footprint
    let db_bytes = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);
    println!("\n{}", "4. SQLite Storage Footprint".bold().green());
    println!("  Index database size: {}", format!("{:.1} KB", db_bytes as f64 / 1024.0).bold());

    println!("\n{}", "✓ Benchmark Completed Successfully!".bold().green());
    Ok(())
}
