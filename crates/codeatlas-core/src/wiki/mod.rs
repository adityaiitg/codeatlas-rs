use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use rusqlite::Connection;

use crate::models::Symbol;

pub struct WikiGenerator<'a> {
    conn: &'a Connection,
    output_dir: PathBuf,
}

impl<'a> WikiGenerator<'a> {
    pub fn new<P: AsRef<Path>>(conn: &'a Connection, output_dir: P) -> Self {
        Self {
            conn,
            output_dir: output_dir.as_ref().to_path_buf(),
        }
    }

    pub fn generate(&self) -> Result<Vec<PathBuf>> {
        fs::create_dir_all(&self.output_dir)?;
        let modules_dir = self.output_dir.join("modules");
        fs::create_dir_all(&modules_dir)?;

        // 1. Fetch symbols
        let mut sym_stmt = self.conn.prepare(
            "SELECT node_id, file_path, kind, name, parent_id, signature, docstring,
                    source_code, start_line, end_line, source_hash, language
             FROM symbols ORDER BY file_path, start_line",
        )?;

        let sym_rows = sym_stmt.query_map([], |row| {
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

        let mut files_map: BTreeMap<String, Vec<Symbol>> = BTreeMap::new();
        for r in sym_rows {
            let s = r?;
            files_map.entry(s.file_path.clone()).or_default().push(s);
        }

        // 2. Fetch edges
        let mut edge_stmt = self.conn.prepare(
            "SELECT source_id, target_id, edge_type FROM edges",
        )?;
        let edge_rows = edge_stmt.query_map([], |row| {
            let src: String = row.get(0)?;
            let tgt: String = row.get(1)?;
            let etype: String = row.get(2)?;
            Ok((src, tgt, etype))
        })?;

        let mut edges = Vec::new();
        for r in edge_rows {
            edges.push(r?);
        }

        let mut generated_files = Vec::new();

        // Generate Architecture
        let arch_file = self.generate_architecture(&files_map, &edges)?;
        generated_files.push(arch_file);

        // Generate Modules
        for (fpath, syms) in &files_map {
            if let Some(m_file) = self.generate_module(fpath, syms, &edges, &modules_dir)? {
                generated_files.push(m_file);
            }
        }

        // Generate Workflows
        let wf_file = self.generate_workflows(&edges)?;
        generated_files.push(wf_file);

        // Generate Main Index
        let idx_file = self.generate_index(&files_map, &generated_files)?;
        generated_files.insert(0, idx_file);

        Ok(generated_files)
    }

    fn generate_architecture(
        &self,
        files_map: &BTreeMap<String, Vec<Symbol>>,
        edges: &[(String, String, String)],
    ) -> Result<PathBuf> {
        let arch_file = self.output_dir.join("architecture.md");
        let mut mermaid = String::from("```mermaid\nflowchart TD\n");
        let mut module_nodes = BTreeSet::new();
        let mut dep_edges = BTreeSet::new();

        for (src, tgt, etype) in edges {
            if etype == "imports" || etype == "calls" {
                let s = clean_node_id(src);
                let t = clean_node_id(tgt);
                if !s.is_empty() && !t.is_empty() && s != t && s.len() < 40 && t.len() < 40 {
                    module_nodes.insert(s.clone());
                    module_nodes.insert(t.clone());
                    dep_edges.insert((s, t));
                }
            }
        }

        let diagram = if !dep_edges.is_empty() {
            for node in module_nodes.iter().take(25) {
                let clean_name = node.split('_').next_back().unwrap_or(node);
                mermaid.push_str(&format!("    {}[\"{}\"]\n", node, clean_name));
            }
            for (s, t) in dep_edges.iter().take(35) {
                mermaid.push_str(&format!("    {} --> {}\n", s, t));
            }
            mermaid.push_str("```\n");
            mermaid
        } else {
            String::from("_No cross-module relations detected._\n")
        };

        let mut content = format!(
            "# System Architecture\n\n## Overview\nThis living architectural specification is automatically synthesized from the codebase AST and Knowledge Graph.\n\n## Subsystem Dependency Graph\n{}\n\n## Discovered Modules & Components\n| File Path | Defined Symbols | Summary |\n| :--- | :--- | :--- |\n",
            diagram
        );

        for (fpath, syms) in files_map {
            let names: Vec<String> = syms
                .iter()
                .filter(|s| matches!(s.kind, crate::models::SymbolKind::Class | crate::models::SymbolKind::Function))
                .map(|s| format!("`{}`", s.name))
                .take(5)
                .collect();
            let names_str = if names.is_empty() { "Module".to_string() } else { names.join(", ") };
            let rel_path = Path::new(fpath).file_name().and_then(|n| n.to_str()).unwrap_or(fpath);
            let stem = Path::new(fpath).file_stem().and_then(|s| s.to_str()).unwrap_or("mod");
            let summary = syms.iter().find_map(|s| s.docstring.as_ref()).map(|s| s.lines().next().unwrap_or("")).unwrap_or("Source module");

            content.push_str(&format!(
                "| [`{}`](./modules/{}.md) | {} | {} |\n",
                rel_path, stem, names_str, summary
            ));
        }

        fs::write(&arch_file, content).with_context(|| format!("Writing {}", arch_file.display()))?;
        Ok(arch_file)
    }

    fn generate_module(
        &self,
        fpath: &str,
        syms: &[Symbol],
        edges: &[(String, String, String)],
        modules_dir: &Path,
    ) -> Result<Option<PathBuf>> {
        let stem = Path::new(fpath).file_stem().and_then(|s| s.to_str()).unwrap_or("module");
        let mod_file = modules_dir.join(format!("{}.md", stem));

        let file_name = Path::new(fpath).file_name().and_then(|s| s.to_str()).unwrap_or(fpath);
        let file_doc = syms
            .iter()
            .find(|s| s.kind == crate::models::SymbolKind::Module && s.docstring.is_some())
            .and_then(|s| s.docstring.as_deref())
            .unwrap_or("Source implementation module.");

        let mut content = format!(
            "# Module: `{}`\n\n**Source Location:** `{}`\n\n## Description\n{}\n\n## Defined Classes & Functions\n",
            file_name, fpath, file_doc
        );

        for s in syms {
            if matches!(s.kind, crate::models::SymbolKind::Class | crate::models::SymbolKind::Function | crate::models::SymbolKind::Method) {
                let sig = match &s.signature {
                    Some(sg) => format!("\n```{}\n{}\n```\n", s.language, sg),
                    None => String::new(),
                };
                let doc = match &s.docstring {
                    Some(d) => format!("> {}\n", d),
                    None => String::new(),
                };
                content.push_str(&format!(
                    "### `{}` ({})\n- **Lines:** {}–{}\n{}{}\n",
                    s.name,
                    s.kind.as_str(),
                    s.start_line,
                    s.end_line,
                    sig,
                    doc
                ));
            }
        }

        // Outgoing calls from this file
        let mut file_calls = BTreeSet::new();
        for (src, tgt, etype) in edges {
            if src.contains(fpath) && etype == "calls" {
                let clean = tgt.strip_prefix("call:").unwrap_or(tgt);
                file_calls.insert(clean.to_string());
            }
        }

        if !file_calls.is_empty() {
            content.push_str("## Invoked External Symbols\n");
            for call in file_calls {
                content.push_str(&format!("- `{}`\n", call));
            }
            content.push('\n');
        }

        fs::write(&mod_file, content).with_context(|| format!("Writing {}", mod_file.display()))?;
        Ok(Some(mod_file))
    }

    fn generate_workflows(&self, edges: &[(String, String, String)]) -> Result<PathBuf> {
        let wf_file = self.output_dir.join("workflows.md");
        let mut calls_map: BTreeMap<String, Vec<String>> = BTreeMap::new();

        for (src, tgt, etype) in edges {
            if etype == "calls" {
                calls_map.entry(src.clone()).or_default().push(tgt.clone());
            }
        }

        let mut content = String::from(
            "# End-to-End Workflows & Execution Paths\n\nThis document maps execution paths through the call graph.\n\n## Detected Call Sequences\n",
        );

        if calls_map.is_empty() {
            content.push_str("\n_No multi-step call sequences detected in current graph._\n");
        } else {
            for (src, tgts) in calls_map.iter().take(20) {
                let src_name = src.split(':').next_back().unwrap_or(src);
                content.push_str(&format!("### Flow from `{}`\n```mermaid\nsequenceDiagram\n", src_name));
                let clean_src = src_name.replace(['.', '-'], "_");
                for tgt in tgts {
                    let tgt_name = tgt.strip_prefix("call:").unwrap_or(tgt).replace(['.', '-'], "_");
                    content.push_str(&format!("    {}->>{}: invoke\n", clean_src, tgt_name));
                }
                content.push_str("```\n\n");
            }
        }

        fs::write(&wf_file, content).with_context(|| format!("Writing {}", wf_file.display()))?;
        Ok(wf_file)
    }

    fn generate_index(
        &self,
        files_map: &BTreeMap<String, Vec<Symbol>>,
        _generated_files: &[PathBuf],
    ) -> Result<PathBuf> {
        let idx_file = self.output_dir.join("index.md");
        let mut content = String::from(
            "# CodeAtlas Living Wiki\n\nWelcome to the automated architectural wiki synthesized directly from source code ASTs and dependency graphs.\n\n## Core Sections\n- [System Architecture](./architecture.md) — High-level module dependency diagrams and structure\n- [Workflows & Execution](./workflows.md) — Call traces and interaction sequence diagrams\n\n## Indexed Modules\n",
        );

        for fpath in files_map.keys() {
            let stem = Path::new(fpath).file_stem().and_then(|s| s.to_str()).unwrap_or("mod");
            let file_name = Path::new(fpath).file_name().and_then(|s| s.to_str()).unwrap_or(fpath);
            content.push_str(&format!("- [`{}`](./modules/{}.md)\n", file_name, stem));
        }

        fs::write(&idx_file, content).with_context(|| format!("Writing {}", idx_file.display()))?;
        Ok(idx_file)
    }
}

fn clean_node_id(id: &str) -> String {
    let base = id
        .strip_prefix("file:")
        .or_else(|| id.strip_prefix("call:"))
        .or_else(|| id.strip_prefix("symbol:"))
        .unwrap_or(id);
    let stem = Path::new(base).file_stem().and_then(|s| s.to_str()).unwrap_or(base);
    stem.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect()
}
