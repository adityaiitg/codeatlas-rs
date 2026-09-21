# codeatlas-mcp

Model Context Protocol (MCP) stdio server for CodeAtlas code intelligence.

Exposes native tools to AI coding agents (Claude Code, Cursor, OpenCode, Codex, Windsurf):
- `codeatlas_search`: Hybrid code search with 1-hop neighborhood context.
- `codeatlas_symbol_graph`: Inspect callers, callees, and connected context.
- `codeatlas_impact_analysis`: Analyze change blast radius before refactoring.
- `codeatlas_status`: Report index size, symbols, and graph edge counts.

## Installation
```bash
cargo install codeatlas-mcp
```

## AI Agent Configuration
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

See [CodeAtlas Repository](https://github.com/adityaiitg/codeatlas-rs) for full documentation.
