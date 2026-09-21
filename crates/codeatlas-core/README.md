# codeatlas-core

Core code intelligence, multi-threaded tree-sitter AST parsing, SQLite FTS5 BM25 search, and petgraph knowledge graph engine.

## Features
- Multi-threaded AST parser (Python + generic) extracting symbols, classes, methods, and call edges.
- Petgraph-based dependency graph with BFS neighborhood expansion and reverse BFS impact analysis.
- SQLite WAL mode storage with FTS5 BM25 lexical tokenization.
- Multi-signal hybrid retriever (exact symbol match, file stem relevance, noise penalties, coherence boost).
- Living architecture wiki and Mermaid diagram generator.

See [CodeAtlas Repository](https://github.com/adityaiitg/codeatlas-rs) for full documentation.
