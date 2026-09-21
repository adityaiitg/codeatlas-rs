use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Module,
    Class,
    Function,
    Method,
    Interface,
    Type,
    Constant,
    Variable,
}

impl SymbolKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Module => "module",
            Self::Class => "class",
            Self::Function => "function",
            Self::Method => "method",
            Self::Interface => "interface",
            Self::Type => "type",
            Self::Constant => "constant",
            Self::Variable => "variable",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub node_id: String,
    pub file_path: String,
    pub kind: SymbolKind,
    pub name: String,
    pub parent_symbol: Option<String>,
    pub signature: Option<String>,
    pub docstring: Option<String>,
    pub start_line: usize,
    pub end_line: usize,
    pub source_code: String,
    pub content_hash: String,
    pub language: String,
    pub calls: Vec<String>,
    pub referenced_types: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkType {
    Code,
    Docs,
    Test,
}

impl ChunkType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Code => "code",
            Self::Docs => "docs",
            Self::Test => "test",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeChunk {
    pub chunk_id: String,
    pub symbol_id: Option<String>,
    pub file_path: String,
    pub chunk_type: ChunkType,
    pub content: String,
    pub start_line: usize,
    pub end_line: usize,
    pub language: String,
    pub content_hash: String,
    pub is_definition: bool,
    pub identifiers: Vec<String>,
    pub identifier_tokens: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    Defines,
    Calls,
    Imports,
    Inherits,
    TestedBy,
}

impl EdgeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Defines => "defines",
            Self::Calls => "calls",
            Self::Imports => "imports",
            Self::Inherits => "inherits",
            Self::TestedBy => "tested_by",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub source_id: String,
    pub target_id: String,
    pub edge_type: EdgeType,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub chunk_id: String,
    pub symbol_id: Option<String>,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub content: String,
    pub chunk_type: ChunkType,
    pub is_definition: bool,
    pub score: f32,
    pub lexical_rank: Option<usize>,
    pub semantic_rank: Option<usize>,
    pub neighbors: Vec<SymbolNeighbor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolNeighbor {
    pub node_id: String,
    pub kind: String,
    pub name: String,
    pub signature: Option<String>,
    pub file_path: String,
    pub start_line: usize,
}
