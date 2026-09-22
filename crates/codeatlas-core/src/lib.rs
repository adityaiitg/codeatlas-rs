pub mod embedder;
pub mod graph;
pub mod models;
pub mod parser;
pub mod retrieval;
pub mod scanner;
pub mod storage;
pub mod wiki;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use anyhow::{Context, Result};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::embedder::CodeEmbedder;
use crate::graph::CodeGraph;
use crate::models::{SearchResult, Symbol};
use crate::parser::AstParser;
use crate::retrieval::Retriever;
use crate::scanner::FileScanner;
use crate::storage::Database;
use crate::wiki::WikiGenerator;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexReport {
    pub files_scanned: usize,
    pub files_indexed: usize,
    pub symbols_count: usize,
    pub chunks_count: usize,
    pub edges_count: usize,
    pub duration_ms: u128,
}

pub struct Engine {
    pub root_path: PathBuf,
    pub db_path: PathBuf,
    pub db: Database,
    pub graph: CodeGraph,
    pub embedder: Arc<CodeEmbedder>,
}

impl Engine {
    pub fn open<P1: AsRef<Path>, P2: AsRef<Path>>(root: P1, db_path: P2) -> Result<Self> {
        let root_path = root.as_ref().to_path_buf();
        let db_path = db_path.as_ref().to_path_buf();
        let db = Database::open(&db_path)
            .with_context(|| format!("Opening database at {}", db_path.display()))?;

        let mut graph = CodeGraph::new();
        // Populate existing graph from database if available
        if let Ok(symbols) = db.load_all_symbols() {
            for sym in &symbols {
                graph.add_symbol(sym);
            }
        }
        if let Ok(edges) = db.load_all_edges() {
            for edge in &edges {
                graph.add_edge(edge);
            }
        }

        let embedder = Arc::new(CodeEmbedder::new());

        Ok(Self {
            root_path,
            db_path,
            db,
            graph,
            embedder,
        })
    }

    pub fn index(&mut self, full: bool) -> Result<IndexReport> {
        self.index_with_options(full, false)
    }

    pub fn index_with_options(&mut self, full: bool, fast: bool) -> Result<IndexReport> {
        let start = Instant::now();

        if fast {
            let _ = self.db.set_fast_mode(true);
        }

        let manifest = if full {
            std::collections::HashMap::new()
        } else {
            self.db.get_manifest_entries()?
        };

        let scanner = FileScanner::new(&self.root_path);
        let scanned = scanner.scan_with_manifest(&manifest, fast);
        let files_scanned = scanned.len();

        // Determine modified or new files
        let mut files_to_process = Vec::new();
        let mut current_file_paths = std::collections::HashSet::new();

        for file in scanned {
            let rel_path = file.relative_path.clone();
            current_file_paths.insert(rel_path.clone());

            let needs_index = match manifest.get(&rel_path) {
                Some(prev) => prev.content_hash != file.content_hash,
                None => true,
            };

            if needs_index {
                files_to_process.push(file);
            }
        }

        // Detect deleted files
        if !full {
            for old_path in manifest.keys() {
                if !current_file_paths.contains(old_path) {
                    let _ = self.db.remove_file(old_path);
                }
            }
        }

        let files_indexed = files_to_process.len();

        // Multi-threaded AST parsing with Rayon
        let parsed_files: Vec<_> = files_to_process
            .par_iter()
            .filter_map(|file| {
                let content = std::fs::read_to_string(&file.path).ok()?;
                let parser = AstParser::new();
                let (syms, chunks, edges) = parser.parse_file(&file.path, &content, &file.language);
                Some((file, syms, chunks, edges))
            })
            .collect();

        let mut total_syms = 0;
        let mut total_chunks = 0;
        let mut total_edges = 0;
        let mut chunks_to_embed: Vec<(String, String)> = Vec::new();

        for (file, syms, chunks, edges) in parsed_files {
            total_syms += syms.len();
            total_chunks += chunks.len();
            total_edges += edges.len();

            if !fast {
                for c in &chunks {
                    let text = format!("{}: {}", c.file_path, c.content);
                    chunks_to_embed.push((c.chunk_id.clone(), text));
                }
            }

            // Insert into SQLite database
            self.db.insert_symbols(&syms)?;
            self.db.insert_chunks(&chunks)?;
            self.db.insert_edges(&edges)?;

            // Update in-memory graph
            for s in &syms {
                self.graph.add_symbol(s);
            }
            for e in &edges {
                self.graph.add_edge(e);
            }

            self.db.update_manifest(
                &file.relative_path,
                &file.content_hash,
                file.mtime,
                file.size,
                &file.language,
            )?;
        }

        // Post-indexing graph linking: resolve call:/symbol: placeholder edges
        // to canonical node IDs, fixing the 0-callers bug in impact analysis.
        if files_indexed > 0 {
            self.graph.link();

            // Semantic vector embeddings for newly indexed chunks (skipped in fast mode)
            if !fast && !chunks_to_embed.is_empty() {
                for chunk_batch in chunks_to_embed.chunks(64) {
                    let texts: Vec<&str> = chunk_batch.iter().map(|(_, t)| t.as_str()).collect();
                    match self.embedder.embed(&texts) {
                            Ok(vectors) => {
                                let pairs: Vec<(String, Vec<f32>)> = chunk_batch
                                    .iter()
                                    .zip(vectors)
                                    .map(|((cid, _), vec)| (cid.clone(), vec))
                                    .collect();
                                if let Err(e) = self.db.insert_embeddings(&pairs) {
                                    tracing::warn!("Failed to store chunk embeddings: {}", e);
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Semantic embedding skipped: {}", e);
                                break;
                            }
                        }
                    }
                }
            }

        if fast {
            let _ = self.db.set_fast_mode(false);
        }

        let duration_ms = start.elapsed().as_millis();

        Ok(IndexReport {
            files_scanned,
            files_indexed,
            symbols_count: total_syms,
            chunks_count: total_chunks,
            edges_count: total_edges,
            duration_ms,
        })
    }

    pub fn search(&self, query: &str, limit: usize, expand_graph: bool) -> Result<Vec<SearchResult>> {
        self.search_with_options(query, limit, expand_graph, false)
    }

    pub fn search_with_options(
        &self,
        query: &str,
        limit: usize,
        expand_graph: bool,
        fast: bool,
    ) -> Result<Vec<SearchResult>> {
        let retriever = Retriever::with_embedder(self.db.conn(), &self.graph, &self.embedder);
        retriever.search_with_options(query, limit, expand_graph, fast)
    }

    pub fn impact(&self, target_id: &str) -> Vec<String> {
        self.graph.impact_analysis(target_id)
    }

    pub fn generate_wiki<P: AsRef<Path>>(&self, output_dir: P) -> Result<Vec<PathBuf>> {
        let generator = WikiGenerator::new(self.db.conn(), output_dir);
        generator.generate()
    }

    pub fn export_graph_json(&self) -> serde_json::Value {
        self.graph.export_json()
    }

    pub fn export_graph_dot(&self) -> String {
        self.graph.export_dot()
    }

    pub fn get_symbols(&self) -> Result<Vec<Symbol>> {
        self.db.load_all_symbols()
    }
}
