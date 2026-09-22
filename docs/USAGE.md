# CodeAtlas (Rust) — Complete Usage Guide ⚡

CodeAtlas is an ultra-fast codebase intelligence engine and deterministic knowledge graph written in Rust. It indexes repositories in milliseconds, provides multi-signal hybrid code search with 1-hop graph neighborhood expansion, conducts instant reverse BFS change impact (blast radius) analysis, generates living architecture wikis with Mermaid diagrams, and integrates natively with AI coding agents via CLI and Model Context Protocol (MCP).

---

## Table of Contents

1. [Installation & Setup](#1-installation--setup)
   - [Cargo (Crates.io)](#cargo-cratesio)
   - [Pre-built Standalone Binaries (GitHub Releases)](#pre-built-standalone-binaries-github-releases)
   - [Building From Source](#building-from-source)
   - [Verifying Installation](#verifying-installation)
2. [CLI Command Reference](#2-cli-command-reference)
   - [`codeatlas index`](#codeatlas-index)
   - [`codeatlas search`](#codeatlas-search)
   - [`codeatlas impact`](#codeatlas-impact)
   - [`codeatlas wiki`](#codeatlas-wiki)
   - [`codeatlas graph`](#codeatlas-graph)
   - [`codeatlas install`](#codeatlas-install)
3. [AI Agent & Editor Skill Integration](#3-ai-agent--editor-skill-integration)
   - [Why Use CLI Skills for Coding Agents?](#why-use-cli-skills-for-coding-agents)
   - [One-Command Skill Installation](#one-command-skill-installation)
   - [Antigravity (AGY)](#antigravity-agy)
   - [Cursor](#cursor)
   - [GitHub Copilot](#github-copilot)
   - [Claude Code, Windsurf & Other Agents](#claude-code-windsurf--other-agents)
4. [Model Context Protocol (MCP) Server](#4-model-context-protocol-mcp-server)
   - [Starting the MCP Server](#starting-the-mcp-server)
   - [MCP Client Configurations](#mcp-client-configurations)
   - [Available MCP Tools](#available-mcp-tools)
5. [Rust Crate API (`codeatlas-core`)](#5-rust-crate-api-codeatlas-core)
   - [Adding to `Cargo.toml`](#adding-to-cargotoml)
   - [Programmatic Examples](#programmatic-examples)
6. [Best Practices & CI/CD Automation](#6-best-practices--cicd-automation)
   - [Git Configuration](#git-configuration)
   - [Automated Architecture Wiki in CI](#automated-architecture-wiki-in-ci)
7. [Troubleshooting & FAQ](#7-troubleshooting--faq)

---

## 1. Installation & Setup

### Cargo (Crates.io)

Both the CLI and MCP server are published on crates.io:

```bash
# Install the codeatlas CLI binary
cargo install codeatlas

# Install the MCP stdio server
cargo install codeatlas-mcp
```

### Pre-built Standalone Binaries (GitHub Releases)

Precompiled, static binaries with zero runtime dependencies are published for every release at [github.com/adityaiitg/codeatlas-rs/releases](https://github.com/adityaiitg/codeatlas-rs/releases):

#### macOS (Apple Silicon / ARM64)
```bash
curl -LO https://github.com/adityaiitg/codeatlas-rs/releases/download/v0.1.0/codeatlas-darwin-arm64.tar.gz
tar -xzf codeatlas-darwin-arm64.tar.gz
sudo mv codeatlas-darwin-arm64/codeatlas /usr/local/bin/
sudo mv codeatlas-darwin-arm64/codeatlas-mcp /usr/local/bin/
rm -rf codeatlas-darwin-arm64*
```

#### Linux (x86_64)
```bash
curl -LO https://github.com/adityaiitg/codeatlas-rs/releases/download/v0.1.0/codeatlas-linux-x86_64.tar.gz
tar -xzf codeatlas-linux-x86_64.tar.gz
sudo mv codeatlas-linux-x86_64/codeatlas /usr/local/bin/
sudo mv codeatlas-linux-x86_64/codeatlas-mcp /usr/local/bin/
rm -rf codeatlas-linux-x86_64*
```

### Building From Source

```bash
git clone https://github.com/adityaiitg/codeatlas-rs.git
cd codeatlas-rs
cargo build --release

# Binaries will be available at:
# ./target/release/codeatlas
# ./target/release/codeatlas-mcp

# Optionally install into Cargo bin:
cargo install --path crates/codeatlas-cli
cargo install --path crates/codeatlas-mcp
```

### Verifying Installation

Ensure the binary is in your `PATH`:

```bash
codeatlas --version
# Output: codeatlas 0.1.0

codeatlas --help
```

---

## 2. CLI Command Reference

### `codeatlas index`

Indexes a codebase into SQLite (`.codeatlas/index.db`) using parallel AST parsing via Rayon and stores symbol metadata, chunks, and graph edges.

```bash
# Index current directory with hybrid semantic embeddings (dense vectors + BM25)
codeatlas index .

# Skip vector embeddings generation (lexical index only)
codeatlas index --no-embeddings .

# Ultra-fast mode: uses mtime/size metadata caching to skip unchanged file I/O
codeatlas index --fast .

# Force a full re-index (bypass incremental hash cache)
codeatlas index . --full

# Index a specific target directory
codeatlas index /path/to/project

# Use a custom database path
codeatlas index . --db /tmp/custom-index.db
```

**Options:**
- `[PATH]`: Root path of the codebase to index (default: `.`)
- `-f, --fast`: Enable ultra-fast indexing mode (metadata-only cache check & in-memory WAL write buffer, skips dense embeddings)
- `--no-embeddings`: Skip generating semantic dense vector embeddings (lexical indexing only)
- `--full`: Force re-indexing all files regardless of mtime/content hash
- `--db <DB>`: Custom path to SQLite index database (default: `.codeatlas/index.db`)

**Example Output:**
```text
⚡ CodeAtlas (Rust) [Hybrid Semantic]
  Indexing directory: /Volumes/T7/Personal_MAC_DATA/Personal/github/codeatlas-rs
  Database target:    /Volumes/T7/Personal_MAC_DATA/Personal/github/codeatlas-rs/.codeatlas/index.db

✓ Indexing Complete
  Files scanned:   18
  Files indexed:   18
  Symbols parsed:  74
  Chunks indexed:  92
  Graph edges:     64
  Vectors embedded:92
  Total latency:   142 ms
```

---

### `codeatlas search`

Performs multi-signal hybrid code search using ONNX Runtime (`ort`) dense vector embeddings combined with SQLite FTS5 BM25 lexical ranking via Reciprocal Rank Fusion (RRF), symbol definitions boosting, path noise damping, and optional 1-hop graph neighborhood expansion.

```bash
# Standard hybrid search query (BM25 + ONNX dense semantic vectors)
codeatlas search "CodeGraph"

# Natural language semantic query
codeatlas search "how is authentication handled"

# Fast sub-millisecond lexical search (direct BM25 scoring without embeddings or graph)
codeatlas search "CodeGraph" --fast

# Disable semantic search (use pure lexical BM25)
codeatlas search "CodeGraph" --no-semantic

# Attach 1-hop graph neighborhood (callers, callees, and imports)
codeatlas search "Retriever" --expand-graph

# Limit number of returned results (default: 10)
codeatlas search "parse_file" --limit 5

# Output machine-readable JSON (ideal for AI agents and automation scripts)
codeatlas search "Database" --json
```

**Options:**
- `<QUERY>`: Search string or symbol name
- `--fast`: Fast search mode (skips dense embeddings and graph neighborhood expansion for sub-millisecond latency)
- `--no-semantic`: Disable semantic dense vector search (pure lexical BM25 ranking)
- `--limit <LIMIT>`: Maximum number of results to display (default: `10`)
- `--expand-graph`: Attach 1-hop call graph and import neighborhood to top matches
- `--json`: Format output as JSON
- `--db <DB>`: Path to index database

**Example Output:**
```text
CodeAtlas Search for "CodeGraph" (4 results):

1. crates/codeatlas-core/src/lib.rs 1:189 (score: 0.0197) 
   Symbol: file:.../crates/codeatlas-core/src/lib.rs
   │ pub mod graph;
   │ pub mod models;
   │ pub mod parser;
   │ pub mod retrieval;

2. crates/codeatlas-core/src/retrieval/mod.rs 1:218 (score: 0.0190) 
   Symbol: file:.../crates/codeatlas-core/src/retrieval/mod.rs
   │ use std::path::Path;
   │ use regex::Regex;
   │ use rusqlite::{params, Connection};
```

---

### `codeatlas impact`

Performs reverse BFS traversal over the code knowledge graph to determine the change blast radius before you modify or refactor a function, class, or symbol.

```bash
# Analyze impact of modifying a symbol
codeatlas impact "CodeGraph"

# Analyze impact on a specific method or struct
codeatlas impact "AstParser"

# Output as JSON
codeatlas impact "Database" --json
```

**Options:**
- `<TARGET>`: Symbol name or symbol ID to evaluate
- `--json`: Output results as JSON array of dependent node IDs
- `--db <DB>`: Custom database path

**Example Output:**
```text
Reverse Impact Analysis for "Retriever" (6 callers/dependents):

  ← file:/project/benchmarks/compare_semble.py
  ← file:/project/src/codeatlas/cli.py
  ← file:/project/src/codeatlas/mcp/server.py
  ← file:/project/tests/unit/test_retriever.py
```

---

### `codeatlas wiki`

Synthesizes a living architecture documentation directory with comprehensive Markdown chapters and auto-generated Mermaid flowcharts and sequence diagrams.

```bash
# Generate architecture wiki in default directory (.codeatlas/wiki)
codeatlas wiki .codeatlas/wiki

# Generate in a documentation folder
codeatlas wiki docs/architecture
```

**Generated Documentation Chapters:**
- `index.md`: Table of contents, overall repository summary, and high-level structure.
- `architecture.md`: Module dependency flowchart (Mermaid `graph TD`), key structs, and subsystems.
- `workflows.md`: Execution sequence diagrams (Mermaid `sequenceDiagram`) showing interactions.
- `modules/*.md`: Detailed per-module documentation including exported symbols, imports, and calls.

---

### `codeatlas graph`

Inspects or exports the in-memory knowledge graph.

```bash
# Print knowledge graph statistics (files, symbols, chunks, edges)
codeatlas graph stats

# Export the graph to Graphviz DOT format
codeatlas graph export --format dot -o graph.dot

# Export the graph to JSON format
codeatlas graph export --format json -o graph.json
```

**Example Output:**
```json
{
  "chunks": 195,
  "edges": 1153,
  "files": 55,
  "symbols": 195
}
```

---

### `codeatlas graph symbol`

Inspects a specific symbol's signature, docstring, 1-hop graph neighbors, and incoming callers:

```bash
# Inspect a symbol
codeatlas graph symbol PaymentProcessor
```

---

### `codeatlas watch`

Background file watcher that runs continuous sub-millisecond incremental indexing on save:

```bash
# Watch current codebase with default 3-second polling interval
codeatlas watch .

# Custom polling interval
codeatlas watch . -i 2
```

---

### `codeatlas hook`

Installs or removes Git hooks (`post-commit`, `post-checkout`, `post-merge`) so every commit or branch switch automatically updates the CodeAtlas index in the background:

```bash
# Install git hooks
codeatlas hook install

# Remove git hooks
codeatlas hook uninstall
```

---

### `codeatlas install`

One-click automated configuration of the CodeAtlas Model Context Protocol (MCP) server for supported AI coding agents.

```bash
# Configure all detected coding assistants
codeatlas install all

# Configure specific assistant
codeatlas install claude    # Configures ~/.claude.json
codeatlas install cursor    # Configures .cursor/mcp.json
codeatlas install opencode  # Configures ~/.opencode/config.json
codeatlas install codex     # Configures ~/.codex/mcp.json
```

---

## 3. AI Agent & Editor Skill Integration

### Why Use CLI Skills for Coding Agents?

Traditional AI agents rely on `grep` or dumping whole files into their context window, which wastes context tokens and frequently suffers from truncation.

| Task | Standard AI Tool | Why `codeatlas` is Superior |
| :--- | :--- | :--- |
| **Locating a symbol** | `grep -rn "def foo"` | Ranks exact symbol definition **first**; filters out duplicates and test files. |
| **Exploring architecture** | Reading files one by one | Indexes in **~50-100ms**; generates Mermaid diagrams automatically. |
| **Refactoring code** | Guessing / grep callers | `codeatlas impact "<symbol>"` performs reverse BFS and reveals **all breaking dependents**. |
| **Context gathering** | Dumping whole files | 40-line chunks preserve token budget with **0% truncation**. |

### One-Command Skill Installation

Every repository contains an automated installer script:

```bash
# Run from repository root to configure global & project skills:
./scripts/install-skill.sh

# Or install for a different project directory:
./scripts/install-skill.sh /path/to/other-project
```

### Antigravity (AGY)

CodeAtlas integrates natively with Google DeepMind's Antigravity (AGY) CLI and IDE via global skills:

1. Global skill location: `~/.gemini/config/skills/codeatlas/SKILL.md`
2. The agent automatically activates the skill whenever you ask queries like:
   - *"Where is the CodeGraph struct defined?"*
   - *"What breaks if I change this function?"*
   - *"Generate an architecture overview for this repo."*

### Cursor

Add `.cursor/rules/codeatlas.mdc` to your project root. Cursor Composer and Chat will automatically follow these rules to use `codeatlas` CLI commands before modifying symbols or exploring the codebase.

### GitHub Copilot

Add `.github/copilot-instructions.md` to your repository. GitHub Copilot Workspace, CLI, and VS Code extensions will leverage `codeatlas` for semantic navigation and blast-radius analysis.

### Claude Code, Windsurf & Other Agents

You can use the universal agent standard at `.agents/skills/codeatlas/SKILL.md` or invoke `codeatlas` directly in terminal tasks.

---

## 4. Model Context Protocol (MCP) Server

### Starting the MCP Server

`codeatlas-mcp` is a standard JSON-RPC 2.0 stdio server:

```bash
codeatlas-mcp
```

### MCP Client Configurations

#### Claude Desktop (`~/Library/Application Support/Claude/claude_desktop_config.json` on macOS or `%APPDATA%\Claude\claude_desktop_config.json` on Windows):
```json
{
  "mcpServers": {
    "codeatlas": {
      "command": "codeatlas-mcp",
      "args": []
    }
  }
}
```

#### Cursor (`.cursor/mcp.json`):
```json
{
  "mcpServers": {
    "codeatlas": {
      "command": "codeatlas-mcp",
      "args": []
    }
  }
}
```

### Available MCP Tools

When connected via MCP, the following tools are available to your agent:

1. `codeatlas_search`: Multi-signal code search with 1-hop graph neighborhood.
2. `codeatlas_symbol_graph`: Inspect incoming callers and outgoing calls for any symbol.
3. `codeatlas_impact_analysis`: Reverse BFS blast-radius analysis.
4. `codeatlas_status`: Report index health, symbol count, and edge statistics.

---

## 5. Rust Crate API (`codeatlas-core`)

You can embed CodeAtlas directly into your own Rust applications without going through the CLI.

### Adding to `Cargo.toml`

```toml
[dependencies]
codeatlas-core = "0.1.0"
```

### Programmatic Examples

```rust
use std::path::Path;
use anyhow::Result;
use codeatlas_core::Engine;

fn main() -> Result<()> {
    let repo_dir = Path::new(".");
    let db_path = Path::new(".codeatlas/index.db");

    // Open or create the index database and in-memory graph
    let mut engine = Engine::open(repo_dir, db_path)?;

    // 1. Index the repository (false = incremental, true = full)
    let report = engine.index(false)?;
    println!(
        "Indexed {} files, found {} symbols in {} ms",
        report.files_indexed, report.symbols_count, report.duration_ms
    );

    // 2. Perform multi-signal search
    let results = engine.search("Engine", 10, true)?;
    for res in results {
        println!("{}:{} [{}]", res.chunk.file_path, res.chunk.start_line, res.score);
    }

    // 3. Impact analysis (reverse BFS)
    let dependents = engine.impact("Engine");
    println!("Dependents of Engine: {:?}", dependents);

    // 4. Generate living documentation wiki
    let wiki_files = engine.generate_wiki(".codeatlas/wiki")?;
    println!("Generated {} wiki chapters", wiki_files.len());

    Ok(())
}
```

---

## 6. Best Practices & CI/CD Automation

### Git Configuration

Always ignore the `.codeatlas/` cache directory in `.gitignore`:

```gitignore
# CodeAtlas index database and cache
.codeatlas/
```

### Automated Architecture Wiki in CI

Add a GitHub Actions workflow (`.github/workflows/codeatlas-wiki.yml`) to keep your living architecture wiki synchronized with `main`:

```yaml
name: Generate Architecture Wiki

on:
  push:
    branches: [main]

jobs:
  wiki:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable

      - name: Install CodeAtlas
        run: cargo install codeatlas

      - name: Index & Generate Wiki
        run: |
          codeatlas index --full .
          codeatlas wiki docs/architecture

      - name: Commit updated documentation
        uses: stefanzweifel/git-auto-commit-action@v5
        with:
          commit_message: "docs(wiki): update automated architecture wiki [skip ci]"
          file_pattern: "docs/architecture/*.md"
```

---

## 7. Troubleshooting & FAQ

### Q: Why do search results return fewer matches after edits?
Run `codeatlas index .` to allow incremental indexing (~6ms) to update modified files. If files were modified externally or renamed, run `codeatlas index . --full`.

### Q: Does CodeAtlas support languages other than Rust?
Yes! CodeAtlas includes AST and symbol parsers for:
- Python (`.py`)
- Rust (`.rs`)
- TypeScript / JavaScript (`.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs`)
- Go (`.go`)
- Java (`.java`)
- C / C++ (`.c`, `.h`, `.cpp`, `.hpp`, `.cc`, `.cxx`)

### Q: How do I completely reset the index?
Delete the `.codeatlas/` directory in your project root:
```bash
rm -rf .codeatlas/
codeatlas index --full .
```

### Q: How does reverse BFS impact analysis handle dynamic dispatch?
CodeAtlas performs static AST symbol resolution. Static calls, explicit method invocations, imports, and definitions are fully resolved with zero false positives.
