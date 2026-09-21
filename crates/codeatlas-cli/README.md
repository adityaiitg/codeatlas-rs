# codeatlas

Ultra-fast code intelligence and knowledge graph CLI engine written in Rust.

## Features
- **Cold Indexing in ~100ms**: Parse AST symbols, chunks, and call graph edges in parallel.
- **Incremental Indexing in ~6ms**: Skip unchanged files with SHA-256 manifest tracking.
- **Sub-millisecond Search**: SQLite FTS5 BM25 with code-aware Reciprocal Rank Fusion.
- **Change Impact Analysis**: Blast radius detection using reverse BFS over call graphs.
- **Living Architecture Wiki**: Synthesizes architecture documentation with Mermaid diagrams.
- **One-Click Agent Setup**: Auto-configures MCP in Claude Code, Cursor, OpenCode, and Codex.

## Installation
```bash
cargo install codeatlas
```

## Quickstart
```bash
codeatlas index .
codeatlas search "function_name"
codeatlas impact "symbol_name"
codeatlas wiki
codeatlas install all
```

See [GitHub Repository](https://github.com/adityaiitg/codeatlas-rs) for full documentation.
