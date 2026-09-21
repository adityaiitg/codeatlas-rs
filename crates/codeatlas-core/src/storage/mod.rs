use std::path::Path;
use anyhow::{Context, Result};
use rusqlite::{params, Connection};

use crate::models::{CodeChunk, Edge, Symbol};

pub const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS symbols (
    node_id      TEXT PRIMARY KEY,
    file_path    TEXT NOT NULL,
    kind         TEXT NOT NULL,
    name         TEXT NOT NULL,
    parent_id    TEXT,
    signature    TEXT,
    docstring    TEXT,
    source_code  TEXT,
    start_line   INTEGER NOT NULL,
    end_line     INTEGER NOT NULL,
    source_hash  TEXT NOT NULL,
    language     TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS edges (
    source_id    TEXT NOT NULL,
    target_id    TEXT NOT NULL,
    edge_type    TEXT NOT NULL,
    metadata     TEXT,
    PRIMARY KEY (source_id, target_id, edge_type)
);

CREATE TABLE IF NOT EXISTS chunks (
    chunk_id          TEXT PRIMARY KEY,
    symbol_id         TEXT,
    file_path         TEXT NOT NULL,
    chunk_type        TEXT NOT NULL,
    content           TEXT NOT NULL,
    start_line        INTEGER NOT NULL,
    end_line          INTEGER NOT NULL,
    language          TEXT NOT NULL,
    content_hash      TEXT NOT NULL,
    is_definition     BOOLEAN NOT NULL,
    identifiers       TEXT NOT NULL,
    identifier_tokens TEXT NOT NULL
);

CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
    chunk_id UNINDEXED,
    file_path,
    content,
    identifiers,
    identifier_tokens,
    tokenize = 'porter unicode61'
);

CREATE TABLE IF NOT EXISTS file_manifest (
    file_path    TEXT PRIMARY KEY,
    content_hash TEXT NOT NULL,
    mtime        REAL,
    size         INTEGER,
    language     TEXT,
    indexed_at   TEXT
);

CREATE INDEX IF NOT EXISTS idx_symbols_file ON symbols(file_path);
CREATE INDEX IF NOT EXISTS idx_symbols_kind ON symbols(kind);
CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);
CREATE INDEX IF NOT EXISTS idx_edges_source ON edges(source_id);
CREATE INDEX IF NOT EXISTS idx_edges_target ON edges(target_id);
CREATE INDEX IF NOT EXISTS idx_chunks_file ON chunks(file_path);
CREATE INDEX IF NOT EXISTS idx_chunks_symbol ON chunks(symbol_id);
"#;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ManifestEntry {
    pub file_path: String,
    pub content_hash: String,
    pub mtime: f64,
    pub size: u64,
    pub language: Option<String>,
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open SQLite database at {}", path.display()))?;

        let _mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0)).unwrap_or_default();
        let _ = conn.execute("PRAGMA synchronous = NORMAL", []);
        let _ = conn.execute("PRAGMA foreign_keys = ON", []);
        let _ = conn.busy_timeout(std::time::Duration::from_millis(5000));

        conn.execute_batch(SCHEMA_SQL)?;

        Ok(Self { conn })
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(SCHEMA_SQL)?;
        Ok(Self { conn })
    }

    pub fn insert_symbols(&mut self, symbols: &[Symbol]) -> Result<()> {
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT OR REPLACE INTO symbols (
                    node_id, file_path, kind, name, parent_id, signature,
                    docstring, source_code, start_line, end_line, source_hash, language
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            )?;

            for sym in symbols {
                stmt.execute(params![
                    sym.node_id,
                    sym.file_path,
                    sym.kind.as_str(),
                    sym.name,
                    sym.parent_symbol,
                    sym.signature,
                    sym.docstring,
                    sym.source_code,
                    sym.start_line as i64,
                    sym.end_line as i64,
                    sym.content_hash,
                    sym.language,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn insert_edges(&mut self, edges: &[Edge]) -> Result<()> {
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT OR IGNORE INTO edges (source_id, target_id, edge_type, metadata)
                 VALUES (?1, ?2, ?3, ?4)",
            )?;

            for edge in edges {
                let meta_str = edge.metadata.as_ref().map(|m| m.to_string());
                stmt.execute(params![
                    edge.source_id,
                    edge.target_id,
                    edge.edge_type.as_str(),
                    meta_str,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn insert_chunks(&mut self, chunks: &[CodeChunk]) -> Result<()> {
        let tx = self.conn.transaction()?;
        {
            let mut chunk_stmt = tx.prepare_cached(
                "INSERT OR REPLACE INTO chunks (
                    chunk_id, symbol_id, file_path, chunk_type, content,
                    start_line, end_line, language, content_hash, is_definition,
                    identifiers, identifier_tokens
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            )?;

            let mut fts_del_stmt = tx.prepare_cached("DELETE FROM chunks_fts WHERE chunk_id = ?1")?;
            let mut fts_ins_stmt = tx.prepare_cached(
                "INSERT INTO chunks_fts (chunk_id, file_path, content, identifiers, identifier_tokens)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;

            for chunk in chunks {
                let id_json = serde_json::to_string(&chunk.identifiers)?;
                let token_json = serde_json::to_string(&chunk.identifier_tokens)?;
                let id_space = chunk.identifiers.join(" ");
                let token_space = chunk.identifier_tokens.join(" ");

                chunk_stmt.execute(params![
                    chunk.chunk_id,
                    chunk.symbol_id,
                    chunk.file_path,
                    chunk.chunk_type.as_str(),
                    chunk.content,
                    chunk.start_line as i64,
                    chunk.end_line as i64,
                    chunk.language,
                    chunk.content_hash,
                    chunk.is_definition,
                    id_json,
                    token_json,
                ])?;

                fts_del_stmt.execute(params![chunk.chunk_id])?;
                fts_ins_stmt.execute(params![
                    chunk.chunk_id,
                    chunk.file_path,
                    chunk.content,
                    id_space,
                    token_space,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn search_lexical(&self, query: &str, limit: usize) -> Result<Vec<(String, f32)>> {
        let clean_q: String = query
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '_' { c } else { ' ' })
            .collect();
        let terms: Vec<&str> = clean_q.split_whitespace().filter(|t| t.len() >= 2).collect();

        if terms.is_empty() {
            return Ok(Vec::new());
        }

        let fts_query = terms
            .iter()
            .map(|t| format!("\"{}\"*", t))
            .collect::<Vec<_>>()
            .join(" OR ");

        let mut stmt = self.conn.prepare(
            "SELECT chunk_id, bm25(chunks_fts) as rank
             FROM chunks_fts
             WHERE chunks_fts MATCH ?1
             ORDER BY rank ASC
             LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![fts_query, limit as i64], |row| {
            let chunk_id: String = row.get(0)?;
            let rank: f64 = row.get(1)?;
            Ok((chunk_id, rank as f32))
        })?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }

        Ok(results)
    }

    pub fn set_fast_mode(&self, enabled: bool) -> Result<()> {
        if enabled {
            let _ = self.conn.execute("PRAGMA synchronous = OFF", []);
            let _ = self.conn.execute("PRAGMA temp_store = MEMORY", []);
            let _ = self.conn.execute("PRAGMA cache_size = -128000", []);
        } else {
            let _ = self.conn.execute("PRAGMA synchronous = NORMAL", []);
        }
        Ok(())
    }

    pub fn get_manifest_entries(&self) -> Result<std::collections::HashMap<String, ManifestEntry>> {
        let mut stmt = self.conn.prepare("SELECT file_path, content_hash, mtime, size, language FROM file_manifest")?;
        let rows = stmt.query_map([], |row| {
            let path: String = row.get(0)?;
            let hash: String = row.get(1)?;
            let mtime: f64 = row.get(2).unwrap_or(0.0);
            let size: i64 = row.get(3).unwrap_or(0);
            let language: Option<String> = row.get(4).ok();
            Ok((path.clone(), ManifestEntry {
                file_path: path,
                content_hash: hash,
                mtime,
                size: size as u64,
                language,
            }))
        })?;

        let mut map = std::collections::HashMap::new();
        for r in rows {
            let (p, entry) = r?;
            map.insert(p, entry);
        }
        Ok(map)
    }

    pub fn get_manifest(&self) -> Result<std::collections::HashMap<String, String>> {
        let mut stmt = self.conn.prepare("SELECT file_path, content_hash FROM file_manifest")?;
        let rows = stmt.query_map([], |row| {
            let path: String = row.get(0)?;
            let hash: String = row.get(1)?;
            Ok((path, hash))
        })?;

        let mut map = std::collections::HashMap::new();
        for r in rows {
            let (p, h) = r?;
            map.insert(p, h);
        }
        Ok(map)
    }

    pub fn update_manifest(&mut self, file_path: &str, content_hash: &str, mtime: f64, size: u64, language: &str) -> Result<()> {
        let now = chrono_now_str();
        self.conn.execute(
            "INSERT OR REPLACE INTO file_manifest (file_path, content_hash, mtime, size, language, indexed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![file_path, content_hash, mtime, size as i64, language, now],
        )?;
        Ok(())
    }

    pub fn remove_file(&mut self, file_path: &str) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM symbols WHERE file_path = ?1", params![file_path])?;
        tx.execute("DELETE FROM chunks WHERE file_path = ?1", params![file_path])?;
        tx.execute("DELETE FROM chunks_fts WHERE file_path = ?1", params![file_path])?;
        tx.execute("DELETE FROM file_manifest WHERE file_path = ?1", params![file_path])?;
        tx.commit()?;
        Ok(())
    }

    pub fn get_stats(&self) -> Result<serde_json::Value> {
        let files: i64 = self.conn.query_row("SELECT count(*) FROM file_manifest", [], |r| r.get(0)).unwrap_or(0);
        let symbols: i64 = self.conn.query_row("SELECT count(*) FROM symbols", [], |r| r.get(0)).unwrap_or(0);
        let edges: i64 = self.conn.query_row("SELECT count(*) FROM edges", [], |r| r.get(0)).unwrap_or(0);
        let chunks: i64 = self.conn.query_row("SELECT count(*) FROM chunks", [], |r| r.get(0)).unwrap_or(0);

        Ok(serde_json::json!({
            "files": files,
            "symbols": symbols,
            "edges": edges,
            "chunks": chunks,
        }))
    }

    pub fn load_all_symbols(&self) -> Result<Vec<Symbol>> {
        let mut stmt = self.conn.prepare(
            "SELECT node_id, file_path, kind, name, parent_id, signature,
                    docstring, source_code, start_line, end_line, source_hash, language
             FROM symbols",
        )?;

        let rows = stmt.query_map([], |row| {
            let kind_str: String = row.get(2)?;
            let kind = match kind_str.as_str() {
                "class" => crate::models::SymbolKind::Class,
                "function" => crate::models::SymbolKind::Function,
                "method" => crate::models::SymbolKind::Method,
                _ => crate::models::SymbolKind::Module,
            };
            Ok(Symbol {
                node_id: row.get(0)?,
                file_path: row.get(1)?,
                kind,
                name: row.get(3)?,
                parent_symbol: row.get(4)?,
                signature: row.get(5)?,
                docstring: row.get(6)?,
                source_code: row.get(7)?,
                start_line: row.get::<_, i64>(8)? as usize,
                end_line: row.get::<_, i64>(9)? as usize,
                content_hash: row.get(10)?,
                language: row.get(11)?,
                calls: Vec::new(),
                referenced_types: Vec::new(),
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    pub fn load_all_edges(&self) -> Result<Vec<Edge>> {
        let mut stmt = self.conn.prepare(
            "SELECT source_id, target_id, edge_type, metadata FROM edges",
        )?;

        let rows = stmt.query_map([], |row| {
            let etype_str: String = row.get(2)?;
            let etype = match etype_str.as_str() {
                "calls" => crate::models::EdgeType::Calls,
                "imports" => crate::models::EdgeType::Imports,
                "inherits" => crate::models::EdgeType::Inherits,
                "tested_by" => crate::models::EdgeType::TestedBy,
                _ => crate::models::EdgeType::Defines,
            };
            let meta_str: Option<String> = row.get(3)?;
            let metadata = meta_str.and_then(|s| serde_json::from_str(&s).ok());

            Ok(Edge {
                source_id: row.get(0)?,
                target_id: row.get(1)?,
                edge_type: etype,
                metadata,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }
}

fn chrono_now_str() -> String {
    // Basic UTC timestamp string without heavy chrono dependency
    let now = std::time::SystemTime::now();
    let d = now.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    format!("{}.{}", d.as_secs(), d.subsec_millis())
}
