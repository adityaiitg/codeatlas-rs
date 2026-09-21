use std::path::Path;
use sha2::{Digest, Sha256};
use tree_sitter::{Node, Parser};

use crate::models::{ChunkType, CodeChunk, Edge, EdgeType, Symbol, SymbolKind};

pub fn split_identifier(ident: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();

    let chars: Vec<char> = ident.chars().collect();
    for i in 0..chars.len() {
        let c = chars[i];
        if c == '_' || c == '-' || c == '.' || c == ':' {
            if !current.is_empty() {
                words.push(current.to_lowercase());
                current.clear();
            }
        } else if c.is_uppercase() {
            if !current.is_empty() {
                // If previous was lowercase or next is lowercase, start a new word
                let prev_lower = chars.get(i.saturating_sub(1)).map_or(false, |p| p.is_lowercase());
                let next_lower = chars.get(i + 1).map_or(false, |n| n.is_lowercase());
                if prev_lower || next_lower {
                    words.push(current.to_lowercase());
                    current.clear();
                }
            }
            current.push(c);
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        words.push(current.to_lowercase());
    }
    words
}

#[derive(Default)]
pub struct AstParser;

impl AstParser {
    pub fn new() -> Self {
        Self
    }

    pub fn parse_file(&self, file_path: &Path, content: &str, language: &str) -> (Vec<Symbol>, Vec<CodeChunk>, Vec<Edge>) {
        match language {
            "python" => self.parse_python(file_path, content),
            _ => self.parse_generic(file_path, content, language),
        }
    }

    fn parse_python(&self, file_path: &Path, content: &str) -> (Vec<Symbol>, Vec<CodeChunk>, Vec<Edge>) {
        let mut symbols = Vec::new();
        let mut chunks = Vec::new();
        let mut edges = Vec::new();

        let mut parser = Parser::new();
        let lang = tree_sitter_python::LANGUAGE;
        if parser.set_language(&lang.into()).is_err() {
            return self.parse_generic(file_path, content, "python");
        }

        let tree = match parser.parse(content, None) {
            Some(t) => t,
            None => return self.parse_generic(file_path, content, "python"),
        };

        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let file_hash = format!("{:x}", hasher.finalize());

        let file_stem = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("module");
        let module_id = format!("file:{}", file_path.display());

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len().max(1);

        // 1. Module Symbol
        symbols.push(Symbol {
            node_id: module_id.clone(),
            file_path: file_path.to_string_lossy().to_string(),
            kind: SymbolKind::Module,
            name: file_stem.to_string(),
            parent_symbol: None,
            signature: None,
            docstring: None,
            start_line: 1,
            end_line: total_lines,
            source_code: content.to_string(),
            content_hash: file_hash.clone(),
            language: "python".to_string(),
            calls: Vec::new(),
            referenced_types: Vec::new(),
        });

        // Module chunk (header / overview)
        let id_tokens = split_identifier(file_stem);
        let header_lines = lines.iter().take(25).cloned().collect::<Vec<_>>().join("\n");
        chunks.push(CodeChunk {
            chunk_id: format!("chunk:{}", module_id),
            symbol_id: Some(module_id.clone()),
            file_path: file_path.to_string_lossy().to_string(),
            chunk_type: if file_path.to_string_lossy().contains("test") {
                ChunkType::Test
            } else {
                ChunkType::Docs
            },
            content: format!("# {}\n{}", file_path.display(), header_lines),
            start_line: 1,
            end_line: total_lines.min(25),
            language: "python".to_string(),
            content_hash: file_hash.clone(),
            is_definition: false,
            identifiers: vec![file_stem.to_string()],
            identifier_tokens: id_tokens,
        });

        let root_node = tree.root_node();
        let mut cursor = root_node.walk();

        for child in root_node.children(&mut cursor) {
            match child.kind() {
                "class_definition" => {
                    self.extract_class(&child, content, file_path, &module_id, &lines, &mut symbols, &mut chunks, &mut edges);
                }
                "function_definition" => {
                    self.extract_function(&child, content, file_path, &module_id, None, &lines, &mut symbols, &mut chunks, &mut edges);
                }
                "import_statement" | "import_from_statement" => {
                    self.extract_import(&child, content, &module_id, &mut edges);
                }
                _ => {}
            }
        }

        (symbols, chunks, edges)
    }

    fn extract_class(
        &self,
        node: &Node,
        content: &str,
        file_path: &Path,
        parent_id: &str,
        lines: &[&str],
        symbols: &mut Vec<Symbol>,
        chunks: &mut Vec<CodeChunk>,
        edges: &mut Vec<Edge>,
    ) {
        let name_node = match node.child_by_field_name("name") {
            Some(n) => n,
            None => return,
        };
        let class_name = &content[name_node.byte_range()];
        let start_line = node.start_position().row + 1;
        let end_line = node.end_position().row + 1;

        let class_node_id = format!("{}:{}", file_path.display(), class_name);

        let full_src = &content[node.byte_range()];
        let mut hasher = Sha256::new();
        hasher.update(full_src.as_bytes());
        let class_hash = format!("{:x}", hasher.finalize());

        // Extract superclasses
        let mut superclasses = Vec::new();
        if let Some(super_node) = node.child_by_field_name("superclasses") {
            let mut cursor = super_node.walk();
            for arg in super_node.children(&mut cursor) {
                if arg.kind() == "identifier" || arg.kind() == "attribute" {
                    superclasses.push(content[arg.byte_range()].to_string());
                }
            }
        }

        // Add inheritance edges
        for base in &superclasses {
            edges.push(Edge {
                source_id: class_node_id.clone(),
                target_id: format!("symbol:{}", base),
                edge_type: EdgeType::Inherits,
                metadata: None,
            });
        }

        // Parent defines class edge
        edges.push(Edge {
            source_id: parent_id.to_string(),
            target_id: class_node_id.clone(),
            edge_type: EdgeType::Defines,
            metadata: None,
        });

        // Class Symbol
        symbols.push(Symbol {
            node_id: class_node_id.clone(),
            file_path: file_path.to_string_lossy().to_string(),
            kind: SymbolKind::Class,
            name: class_name.to_string(),
            parent_symbol: Some(parent_id.to_string()),
            signature: Some(format!("class {}:", class_name)),
            docstring: None,
            start_line,
            end_line,
            source_code: full_src.to_string(),
            content_hash: class_hash.clone(),
            language: "python".to_string(),
            calls: Vec::new(),
            referenced_types: superclasses,
        });

        // Class Chunk (capped at 40 lines to avoid 512-token truncation)
        let chunk_end_line = start_line + (end_line - start_line).min(40);
        let class_chunk_src = lines[start_line - 1..chunk_end_line.min(lines.len())].join("\n");
        let id_tokens = split_identifier(class_name);

        chunks.push(CodeChunk {
            chunk_id: format!("chunk:{}", class_node_id),
            symbol_id: Some(class_node_id.clone()),
            file_path: file_path.to_string_lossy().to_string(),
            chunk_type: if file_path.to_string_lossy().contains("test") {
                ChunkType::Test
            } else {
                ChunkType::Code
            },
            content: class_chunk_src,
            start_line,
            end_line: chunk_end_line,
            language: "python".to_string(),
            content_hash: class_hash,
            is_definition: true,
            identifiers: vec![class_name.to_string()],
            identifier_tokens: id_tokens,
        });

        // Walk methods inside class body
        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for item in body.children(&mut cursor) {
                if item.kind() == "function_definition" {
                    self.extract_function(&item, content, file_path, &class_node_id, Some(class_name), lines, symbols, chunks, edges);
                }
            }
        }
    }

    fn extract_function(
        &self,
        node: &Node,
        content: &str,
        file_path: &Path,
        parent_id: &str,
        class_name: Option<&str>,
        _lines: &[&str],
        symbols: &mut Vec<Symbol>,
        chunks: &mut Vec<CodeChunk>,
        edges: &mut Vec<Edge>,
    ) {
        let name_node = match node.child_by_field_name("name") {
            Some(n) => n,
            None => return,
        };
        let fn_name = &content[name_node.byte_range()];
        let start_line = node.start_position().row + 1;
        let end_line = node.end_position().row + 1;

        let full_name = match class_name {
            Some(cls) => format!("{}.{}", cls, fn_name),
            None => fn_name.to_string(),
        };
        let fn_node_id = format!("{}:{}", file_path.display(), full_name);

        let fn_src = &content[node.byte_range()];
        let mut hasher = Sha256::new();
        hasher.update(fn_src.as_bytes());
        let fn_hash = format!("{:x}", hasher.finalize());

        let sig = format!("def {}(...):", fn_name);
        let calls = self.extract_calls(node, content);

        for callee in &calls {
            edges.push(Edge {
                source_id: fn_node_id.clone(),
                target_id: format!("call:{}", callee),
                edge_type: EdgeType::Calls,
                metadata: None,
            });
        }

        edges.push(Edge {
            source_id: parent_id.to_string(),
            target_id: fn_node_id.clone(),
            edge_type: EdgeType::Defines,
            metadata: None,
        });

        let is_test = file_path.to_string_lossy().contains("test") || fn_name.starts_with("test_");

        symbols.push(Symbol {
            node_id: fn_node_id.clone(),
            file_path: file_path.to_string_lossy().to_string(),
            kind: if class_name.is_some() {
                SymbolKind::Method
            } else {
                SymbolKind::Function
            },
            name: fn_name.to_string(),
            parent_symbol: Some(parent_id.to_string()),
            signature: Some(sig),
            docstring: None,
            start_line,
            end_line,
            source_code: fn_src.to_string(),
            content_hash: fn_hash.clone(),
            language: "python".to_string(),
            calls,
            referenced_types: Vec::new(),
        });

        let id_tokens = split_identifier(fn_name);
        chunks.push(CodeChunk {
            chunk_id: format!("chunk:{}", fn_node_id),
            symbol_id: Some(fn_node_id),
            file_path: file_path.to_string_lossy().to_string(),
            chunk_type: if is_test { ChunkType::Test } else { ChunkType::Code },
            content: fn_src.to_string(),
            start_line,
            end_line,
            language: "python".to_string(),
            content_hash: fn_hash,
            is_definition: true,
            identifiers: vec![fn_name.to_string()],
            identifier_tokens: id_tokens,
        });
    }

    fn extract_calls(&self, node: &Node, content: &str) -> Vec<String> {
        let mut calls = Vec::new();
        let mut stack = vec![*node];

        while let Some(current) = stack.pop() {
            if current.kind() == "call" {
                if let Some(func_node) = current.child_by_field_name("function") {
                    let call_text = &content[func_node.byte_range()];
                    calls.push(call_text.to_string());
                }
            }

            let mut cursor = current.walk();
            for child in current.children(&mut cursor) {
                stack.push(child);
            }
        }

        calls
    }

    fn extract_import(&self, node: &Node, content: &str, module_id: &str, edges: &mut Vec<Edge>) {
        let text = &content[node.byte_range()];
        let clean = text.trim();
        edges.push(Edge {
            source_id: module_id.to_string(),
            target_id: format!("import:{}", clean),
            edge_type: EdgeType::Imports,
            metadata: Some(serde_json::json!({"statement": clean})),
        });
    }

    fn parse_generic(&self, file_path: &Path, content: &str, language: &str) -> (Vec<Symbol>, Vec<CodeChunk>, Vec<Edge>) {
        let mut symbols = Vec::new();
        let mut chunks = Vec::new();
        let edges = Vec::new();

        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        let lines: Vec<&str> = content.lines().collect();
        let file_stem = file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
        let module_id = format!("file:{}", file_path.display());

        symbols.push(Symbol {
            node_id: module_id.clone(),
            file_path: file_path.to_string_lossy().to_string(),
            kind: SymbolKind::Module,
            name: file_stem.to_string(),
            parent_symbol: None,
            signature: None,
            docstring: None,
            start_line: 1,
            end_line: lines.len().max(1),
            source_code: content.to_string(),
            content_hash: hash.clone(),
            language: language.to_string(),
            calls: Vec::new(),
            referenced_types: Vec::new(),
        });

        chunks.push(CodeChunk {
            chunk_id: format!("chunk:{}", module_id),
            symbol_id: Some(module_id),
            file_path: file_path.to_string_lossy().to_string(),
            chunk_type: ChunkType::Code,
            content: content.to_string(),
            start_line: 1,
            end_line: lines.len().max(1),
            language: language.to_string(),
            content_hash: hash,
            is_definition: false,
            identifiers: vec![file_stem.to_string()],
            identifier_tokens: split_identifier(file_stem),
        });

        (symbols, chunks, edges)
    }
}
