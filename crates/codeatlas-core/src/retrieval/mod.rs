use std::path::Path;
use regex::Regex;
use rusqlite::{params, Connection};

use crate::embedder::CodeEmbedder;
use crate::graph::CodeGraph;
use crate::models::{ChunkType, SearchResult, SymbolNeighbor};

pub struct Retriever<'a> {
    conn: &'a Connection,
    graph: &'a CodeGraph,
    embedder: Option<&'a CodeEmbedder>,
    test_re: Regex,
    compat_re: Regex,
}

impl<'a> Retriever<'a> {
    pub fn new(conn: &'a Connection, graph: &'a CodeGraph) -> Self {
        Self {
            conn,
            graph,
            embedder: None,
            test_re: Regex::new(r"(?:^|[\\/])(?:tests?|__tests__|spec|testing)(?:[\\/]|$)|test_[^\\/]*\.\w+$|[^\\/]*_test\.\w+$|[^\\/]*Tests?\.\w+$|[^\\/]*_spec\.\w+$").unwrap(),
            compat_re: Regex::new(r"(?:^|[\\/])(?:compat|_compat|legacy)(?:[\\/]|$)").unwrap(),
        }
    }

    pub fn with_embedder(conn: &'a Connection, graph: &'a CodeGraph, embedder: &'a CodeEmbedder) -> Self {
        Self {
            conn,
            graph,
            embedder: Some(embedder),
            test_re: Regex::new(r"(?:^|[\\/])(?:tests?|__tests__|spec|testing)(?:[\\/]|$)|test_[^\\/]*\.\w+$|[^\\/]*_test\.\w+$|[^\\/]*Tests?\.\w+$|[^\\/]*_spec\.\w+$").unwrap(),
            compat_re: Regex::new(r"(?:^|[\\/])(?:compat|_compat|legacy)(?:[\\/]|$)").unwrap(),
        }
    }

    pub fn search(&self, query: &str, limit: usize, expand_graph: bool) -> anyhow::Result<Vec<SearchResult>> {
        self.search_with_options(query, limit, expand_graph, false)
    }

    pub fn search_with_options(
        &self,
        query: &str,
        limit: usize,
        expand_graph: bool,
        fast: bool,
    ) -> anyhow::Result<Vec<SearchResult>> {
        let clean_q = query.trim().to_lowercase();
        // Use word-boundary matching for test/spec queries (not substring)
        let is_test_query = {
            let wb_re = Regex::new(r"\b(test|tests|spec|specs)\b").unwrap();
            wb_re.is_match(&clean_q)
        };
        // Detect all-uppercase acronym queries (e.g. "JWT", "HTTP", "CLI")
        let is_acronym = query.trim().chars().all(|c| c.is_uppercase() || c == '_' || c == '-');

        let fts_limit = if fast { limit.max(15) } else { 50 };

        // 1. Query FTS5 BM25 (lexical)
        let clean_tokens: String = query
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '_' { c } else { ' ' })
            .collect();
        let terms: Vec<&str> = clean_tokens.split_whitespace().filter(|t| t.len() >= 2).collect();

        let mut lex_ranks: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        if !terms.is_empty() {
            let fts_query = terms
                .iter()
                .map(|t| format!("\"{}\"*", t))
                .collect::<Vec<_>>()
                .join(" OR ");

            if let Ok(mut stmt) = self.conn.prepare(
                "SELECT chunk_id, bm25(chunks_fts) as rank
                 FROM chunks_fts
                 WHERE chunks_fts MATCH ?1
                 ORDER BY rank ASC
                 LIMIT ?2",
            ) {
                if let Ok(rows) = stmt.query_map(params![fts_query, fts_limit as i64], |row| {
                    let chunk_id: String = row.get(0)?;
                    let rank: f64 = row.get(1)?;
                    Ok((chunk_id, rank as f32))
                }) {
                    for (i, r) in rows.flatten().enumerate() {
                        lex_ranks.insert(r.0, i + 1);
                    }
                }
            }
        }

        // 2. Query Semantic Embeddings (dense vector cosine similarity)
        let mut sem_ranks: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        if !fast {
            if let Some(emb) = self.embedder {
                if let Ok(query_vec) = emb.embed_query(query) {
                    if let Ok(mut sem_stmt) = self.conn.prepare(
                        "SELECT chunk_id, embedding FROM chunk_embeddings",
                    ) {
                        if let Ok(rows) = sem_stmt.query_map([], |row| {
                            let cid: String = row.get(0)?;
                            let blob: Vec<u8> = row.get(1)?;
                            let vec = crate::embedder::bytes_to_embedding(&blob);
                            Ok((cid, vec))
                        }) {
                            let mut similarities: Vec<(String, f32)> = Vec::new();
                            for r in rows.flatten() {
                                let sim = crate::embedder::cosine_similarity(&query_vec, &r.1);
                                if sim > 0.05 {
                                    similarities.push((r.0, sim));
                                }
                            }
                            similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                            for (i, (cid, _)) in similarities.into_iter().take(fts_limit).enumerate() {
                                sem_ranks.insert(cid, i + 1);
                            }
                        }
                    }
                }
            }
        }

        if lex_ranks.is_empty() && sem_ranks.is_empty() {
            return Ok(Vec::new());
        }

        // 3. Combine chunk candidate IDs from lexical and semantic signals
        let mut chunk_ids: Vec<String> = lex_ranks.keys().cloned().collect();
        for cid in sem_ranks.keys() {
            if !lex_ranks.contains_key(cid) {
                chunk_ids.push(cid.clone());
            }
        }

        let placeholders = chunk_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT chunk_id, symbol_id, file_path, start_line, end_line, content, chunk_type, is_definition
             FROM chunks WHERE chunk_id IN ({})",
            placeholders
        );

        let mut meta_stmt = self.conn.prepare(&sql)?;
        let param_values: Vec<&dyn rusqlite::ToSql> = chunk_ids.iter().map(|id| id as &dyn rusqlite::ToSql).collect();

        let chunk_rows = meta_stmt.query_map(param_values.as_slice(), |row| {
            let cid: String = row.get(0)?;
            let sid: Option<String> = row.get(1)?;
            let fpath: String = row.get(2)?;
            let sline: i64 = row.get(3)?;
            let eline: i64 = row.get(4)?;
            let content: String = row.get(5)?;
            let ctype_str: String = row.get(6)?;
            let is_def: bool = row.get(7)?;

            let ctype = match ctype_str.as_str() {
                "test" => ChunkType::Test,
                "docs" => ChunkType::Docs,
                _ => ChunkType::Code,
            };

            Ok((cid, sid, fpath, sline as usize, eline as usize, content, ctype, is_def))
        })?;

        let k = 60.0f32;
        let mut candidates = Vec::new();

        for r in chunk_rows {
            let (cid, sid, fpath, sline, eline, content, ctype, is_def) = r?;
            let l_rank = lex_ranks.get(&cid).copied();
            let s_rank = sem_ranks.get(&cid).copied();

            // Hybrid RRF (Reciprocal Rank Fusion) score
            let mut score = 0.0f32;
            if let Some(lr) = l_rank {
                score += 1.0f32 / (k + lr as f32);
            }
            if let Some(sr) = s_rank {
                score += 1.0f32 / (k + sr as f32);
            }

            // Definition boost
            if is_def {
                score *= 1.25;
            }

            // Code-aware boosts
            if let Some(ref sym) = sid {
                let sym_name = sym.split(':').next_back().unwrap_or("").to_lowercase();
                if sym_name == clean_q || sym_name.ends_with(&format!(".{}", clean_q)) {
                    score *= 2.0;
                }
                // Acronym boost: if query looks like an acronym (e.g. "JWT"), boost symbols containing it
                if is_acronym {
                    let sym_name_raw = sym.split(':').next_back().unwrap_or("");
                    if sym_name_raw.contains(query.trim()) {
                        score *= 1.5;
                    }
                }
            }

            let file_stem = Path::new(&fpath)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();

            if !file_stem.is_empty() {
                if clean_q == file_stem || clean_q.trim_end_matches('s') == file_stem.trim_end_matches('s') {
                    score *= 1.4;
                } else if terms.iter().any(|&t| t == file_stem) {
                    score *= 1.2;
                }
            }

            // Noise penalties
            if !is_test_query {
                if fast {
                    if fpath.contains("test") {
                        score *= 0.35;
                    }
                } else if self.test_re.is_match(&fpath) {
                    score *= 0.35;
                } else if self.compat_re.is_match(&fpath) {
                    score *= 0.5;
                }
            }

            candidates.push(SearchResult {
                chunk_id: cid,
                symbol_id: sid,
                file_path: fpath,
                start_line: sline,
                end_line: eline,
                content,
                chunk_type: ctype,
                is_definition: is_def,
                score,
                lexical_rank: l_rank,
                semantic_rank: s_rank,
                neighbors: Vec::new(),
            });
        }

        // File coherence boost (skipped in fast mode)
        if !fast && !candidates.is_empty() {
            let max_score = candidates.iter().map(|c| c.score).fold(0.0f32, f32::max);
            if max_score > 0.0 {
                let mut file_scores: std::collections::HashMap<String, f32> = std::collections::HashMap::new();
                let mut best_indices: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

                for (idx, c) in candidates.iter().enumerate() {
                    *file_scores.entry(c.file_path.clone()).or_insert(0.0) += c.score;
                    let best = best_indices.entry(c.file_path.clone()).or_insert(idx);
                    if c.score > candidates[*best].score {
                        *best = idx;
                    }
                }

                let max_file_score = file_scores.values().cloned().fold(0.0f32, f32::max);
                let coherence_unit = max_score * 0.2;

                for (fpath, &best_idx) in &best_indices {
                    if let Some(&fscore) = file_scores.get(fpath) {
                        candidates[best_idx].score += coherence_unit * (fscore / max_file_score);
                    }
                }
            }
        }

        candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        candidates.truncate(limit);

        // 3. Attach 1-hop graph neighbors if requested (skipped in fast mode)
        if !fast && expand_graph {
            for res in &mut candidates {
                if let Some(ref sym_id) = res.symbol_id {
                    let neighbors = self.graph.expand_neighborhood(std::slice::from_ref(sym_id), 1);
                    for nid in neighbors {
                        if nid != *sym_id {
                            if let Some(&node_idx) = self.graph.node_indices.get(&nid) {
                                let n = &self.graph.graph[node_idx];
                                res.neighbors.push(SymbolNeighbor {
                                    node_id: n.id.clone(),
                                    kind: n.kind.clone(),
                                    name: n.name.clone(),
                                    signature: None,
                                    file_path: n.file_path.clone(),
                                    start_line: n.start_line,
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(candidates)
    }
}
