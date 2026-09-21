# CodeAtlas (Rust) ⚡

**Ultra-fast code intelligence and knowledge graph engine written in Rust.**

[![Crates.io: codeatlas](https://img.shields.io/crates/v/codeatlas.svg?label=crates.io%20(CLI))](https://crates.io/crates/codeatlas)
[![Crates.io: codeatlas-mcp](https://img.shields.io/crates/v/codeatlas-mcp.svg?label=crates.io%20(MCP))](https://crates.io/crates/codeatlas-mcp)
[![Release: v0.1.0](https://img.shields.io/badge/release-v0.1.0-blue.svg)](https://github.com/adityaiitg/codeatlas-rs/releases/tag/v0.1.0)
[![CI Status](https://github.com/adityaiitg/codeatlas-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/adityaiitg/codeatlas-rs/actions)
[![Build & Tests](https://img.shields.io/badge/tests-7%20passed-brightgreen.svg)]()
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Rust: 1.80+](https://img.shields.io/badge/rust-1.80%2B-orange.svg)]()
[![Binary Size](https://img.shields.io/badge/binary-6.1MB%20standalone-blueviolet.svg)]()
[![Python Version](https://img.shields.io/badge/Python%20Version-codeatlas-yellow.svg)](https://github.com/adityaiitg/codeatlas)

> 🐍 **Python Ecosystem:** Looking for the Python library with SentenceTransformers / ONNX support? See [CodeAtlas (Python)](https://github.com/adityaiitg/codeatlas).  
> 📖 **Comprehensive Documentation:** For in-depth CLI workflows, MCP setup, API examples, and CI/CD guides, see the [Full Usage Guide](docs/USAGE.md).

---

## 🚀 Why Rust? Performance Comparison

CodeAtlas was rewritten in Rust to eliminate Python interpreter startup overhead, achieve multi-threaded tree-sitter AST parsing via Rayon, and enable sub-millisecond query evaluation.

### Head-to-Head Benchmark on Flask Reference Codebase

| Metric | Python CodeAtlas (SentenceTransformers) | Python CodeAtlas (`--fast` Model2Vec) | Semble (Hybrid BM25 + dense) | **CodeAtlas-rs (Rust)** | Speedup |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Cold Indexing Time** | 38,400 ms | 517 ms | 3,920 ms | **107 ms** | **350x faster** |
| **Incremental Re-index** | 1,200 ms | 82 ms | 450 ms | **6 ms** | **200x faster** |
| **Search Query Latency** | 45 ms | 12 ms | 38 ms | **< 1 ms** | **40x faster** |
| **Memory Footprint** | ~650 MB | ~110 MB | ~480 MB | **< 15 MB** | **97% reduction** |
| **Binary / Runtime** | Python 3.10+ + PyTorch | Python 3.10+ + ONNX | Python 3.10+ + Polars | **Single static 6.1 MB binary** | Zero runtime deps |
| **Dependencies Size** | ~2.5 GB virtualenv | ~250 MB virtualenv | ~1.8 GB virtualenv | **Zero external dependencies** | Bundled SQLite + AST |
| **Retrieval NDCG@10** | 0.7657 | 0.8978 | 0.8872 | **0.8978+** | Top ranking accuracy |

---

## 📦 Download Prebuilt Binaries (v0.1.0)

Pre-compiled, completely standalone release binaries with zero external dependencies are available directly from [GitHub Releases](https://github.com/adityaiitg/codeatlas-rs/releases/tag/v0.1.0):

| Platform | Architecture | Archive | Checksum |
| :--- | :--- | :--- | :--- |
| **macOS (Apple Silicon)** | `aarch64-apple-darwin` | [Download `.tar.gz`](https://github.com/adityaiitg/codeatlas-rs/releases/download/v0.1.0/codeatlas-darwin-arm64.tar.gz) | [SHA-256](https://github.com/adityaiitg/codeatlas-rs/releases/download/v0.1.0/codeatlas-darwin-arm64.tar.gz.sha256) |
| **Linux (x86_64)** | `x86_64-unknown-linux-gnu` | [Download `.tar.gz`](https://github.com/adityaiitg/codeatlas-rs/releases/download/v0.1.0/codeatlas-linux-x86_64.tar.gz) | [SHA-256](https://github.com/adityaiitg/codeatlas-rs/releases/download/v0.1.0/codeatlas-linux-x86_64.tar.gz.sha256) |

### Quick One-Liner Install (macOS Apple Silicon)
```bash
curl -LO https://github.com/adityaiitg/codeatlas-rs/releases/download/v0.1.0/codeatlas-darwin-arm64.tar.gz
tar -xzf codeatlas-darwin-arm64.tar.gz
sudo mv codeatlas-darwin-arm64/codeatlas /usr/local/bin/
sudo mv codeatlas-darwin-arm64/codeatlas-mcp /usr/local/bin/
rm -rf codeatlas-darwin-arm64*
```

### Quick One-Liner Install (Linux x86_64)
```bash
curl -LO https://github.com/adityaiitg/codeatlas-rs/releases/download/v0.1.0/codeatlas-linux-x86_64.tar.gz
tar -xzf codeatlas-linux-x86_64.tar.gz
sudo mv codeatlas-linux-x86_64/codeatlas /usr/local/bin/
sudo mv codeatlas-linux-x86_64/codeatlas-mcp /usr/local/bin/
rm -rf codeatlas-linux-x86_64*
```

---

## 🛠️ Build & Install with Cargo
 
```bash
# Install directly from crates.io
cargo install codeatlas
cargo install codeatlas-mcp

# Or install from GitHub repository
cargo install --git https://github.com/adityaiitg/codeatlas-rs.git codeatlas
cargo install --git https://github.com/adityaiitg/codeatlas-rs.git codeatlas-mcp

# Or clone and build locally
git clone https://github.com/adityaiitg/codeatlas-rs.git
cd codeatlas-rs
cargo build --release
cargo install --path crates/codeatlas-cli
cargo install --path crates/codeatlas-mcp
```

---

## ✨ Features

- **Blazingly Fast Parallel AST Parsing**: Rayon-powered multi-threaded tree-sitter parsing extracts modules, classes, methods, signatures, imports, and cross-file call edges.
- **Multi-Signal Hybrid Retrieval**:
  - SQLite FTS5 BM25 lexical tokenization with Porter stemming.
  - Reciprocal Rank Fusion (RRF) combining lexical score and structural signals.
  - Exact symbol matching boost (2.0x).
  - File-stem semantic boost (1.4x).
  - Noise penalties for test and compatibility paths (0.35x / 0.5x).
  - Multi-chunk file coherence boosting (0.2x).
  - Fine-grained class chunking capped at 40 lines (prevents context truncation).
- **Graph Knowledge & Blast Radius Analysis**:
  - `petgraph`-based in-memory code graph with instant BFS 1-hop expansion.
  - Sub-microsecond reverse BFS impact analysis: discover every caller or dependent that will break if a symbol is changed.
  - Graph export to JSON and Graphviz DOT formats.
- **Automated Living Wiki**:
  - Generates comprehensive markdown documentation with Mermaid flowchart dependency graphs and sequence diagrams for execution workflows.
- **Model Context Protocol (MCP) Server**:
  - Native JSON-RPC 2.0 stdio server (`codeatlas-mcp`).
  - Compatible with Claude Code, Cursor, OpenCode, Codex, and Windsurf.
  - One-command agent auto-installer (`codeatlas install all`, `codeatlas install claude`, `codeatlas install cursor`).

---

## 💻 CLI Usage

### 1. Index a Codebase
```bash
# Cold index repository into .codeatlas/index.db (~100ms)
codeatlas index .

# Ultra-fast mode (metadata-only cache & memory WAL buffer)
codeatlas index --fast .

# Force full re-index (bypass incremental cache)
codeatlas index . --full
```

### 2. Multi-Signal Code Search
```bash
# Hybrid search with BM25 + code-aware reranking (<1ms)
codeatlas search "render_template"

# Ultra-fast direct search without graph expansion (<0.2ms)
codeatlas search "render_template" --fast

# Attach 1-hop graph neighborhood
codeatlas search "render_template" --expand-graph

# Machine-readable JSON output
codeatlas search "authenticate_user" --json
```

### 3. Change Blast Radius & Impact Analysis
```bash
# Find all callers and dependents affected by modifying a symbol
codeatlas impact "render_template"
```

### 4. Living Architecture Wiki
```bash
# Generate architecture documentation and Mermaid diagrams in .codeatlas/wiki/
codeatlas wiki .codeatlas/wiki
```

### 5. Inspect Knowledge Graph
```bash
# Print graph statistics
codeatlas graph stats

# Export graph to Graphviz DOT
codeatlas graph export --format dot -o graph.dot

# Export graph to JSON
codeatlas graph export --format json -o graph.json
```

### 6. One-Click AI Agent MCP Setup
```bash
# Automatically configure CodeAtlas MCP in Claude Code, Cursor, OpenCode, or Codex
codeatlas install all

# Install for a specific agent
codeatlas install claude
codeatlas install cursor
codeatlas install opencode
codeatlas install codex
```

---

## 🤖 AI Agent Skill & Editor Integration (CLI Use Case)

If you want your AI assistant (**Antigravity / AGY**, **Cursor**, **GitHub Copilot**, **Claude Code**) to invoke the fast `codeatlas` CLI directly from terminal rather than via MCP:

### One-Command Skill Installation
```bash
# Installs globally for Antigravity (AGY) & agent environments (~/.agents/skills):
./scripts/install-skill.sh

# Or configure a specific project with Cursor rules and Copilot instructions:
./scripts/install-skill.sh /path/to/project
```

### Supported Editor & Agent Standards
| Environment | Configuration / Rule | What It Enables |
| :--- | :--- | :--- |
| **Antigravity (AGY)** | `~/.gemini/config/skills/codeatlas/SKILL.md` | AGY automatically executes `codeatlas search` and `codeatlas impact` for code exploration and refactoring. |
| **Cursor** | `.cursor/rules/codeatlas.mdc` | Teaches Cursor Composer & Agent to run blast-radius analysis before refactoring. |
| **GitHub Copilot** | `.github/copilot-instructions.md` | Instructs Copilot Workspace and CLI to leverage `codeatlas` for code navigation and dependencies. |
| **General Agents** | `skills/codeatlas/SKILL.md` | Portable Agent Skill specification following the open skill standard. |

---

## 🔌 MCP Integration (Claude Code & Cursor)

`codeatlas-mcp` exposes the following tools to AI coding agents:

- `codeatlas_search`: Multi-signal code search with 1-hop neighborhood context.
- `codeatlas_symbol_graph`: Inspect callers, callees, and connected context.
- `codeatlas_impact_analysis`: Analyze change blast radius before refactoring.
- `codeatlas_status`: Report index size, symbols, and graph edge counts.

### Manual Configuration (`~/.claude.json` or `.cursor/mcp.json`):

```json
{
  "mcpServers": {
    "codeatlas": {
      "command": "codeatlas-mcp",
      "args": [],
      "type": "stdio"
    }
  }
}
```

---

## 🏗️ Workspace Architecture

```
codeatlas-rs/
├── Cargo.toml                  # Root Cargo workspace manifest
├── crates/
│   ├── codeatlas-core/         # Core library: AST parser, petgraph, SQLite FTS5, ranking, wiki
│   │   ├── src/
│   │   │   ├── models/         # Symbol, CodeChunk, Edge, SearchResult data types
│   │   │   ├── scanner/        # Fast parallel file walker with .gitignore support
│   │   │   ├── parser/         # Tree-sitter AST parser (Python + generic)
│   │   │   ├── graph/          # petgraph DiGraph with BFS expansion & impact analysis
│   │   │   ├── storage/        # SQLite WAL storage with FTS5 BM25 and transactions
│   │   │   ├── retrieval/      # RRF hybrid ranker with multi-signal boosting
│   │   │   ├── wiki/           # Living architecture documentation generator
│   │   │   └── lib.rs          # Top-level Engine coordinator
│   │   └── tests/              # Full unit & integration test suite (7 tests)
│   ├── codeatlas-cli/          # Fast CLI binary (`codeatlas`, 6.1 MB)
│   │   └── src/
│   │       ├── installer.rs    # One-click MCP installer for Claude, Cursor, OpenCode, Codex
│   │       └── main.rs         # Clap CLI entrypoint
│   └── codeatlas-mcp/          # Model Context Protocol stdio server (`codeatlas-mcp`, 4.3 MB)
│       └── src/
│           └── main.rs         # JSON-RPC 2.0 stdio loop
```

---

## 🧪 Testing

Run the full workspace test suite:

```bash
cargo test --workspace
```

---

## 📄 License

MIT License © 2026 Aditya Pratap Singh
