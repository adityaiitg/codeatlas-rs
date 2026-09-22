use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use anyhow::Result;
use serde_json::{json, Value};

use codeatlas_core::Engine;

const SERVER_NAME: &str = "codeatlas";
const SERVER_VERSION: &str = "0.3.0";
const PROTOCOL_VERSION: &str = "2024-11-05";

fn get_tools_def() -> Value {
    json!([
        {
            "name": "codeatlas_search",
            "description": "Perform hybrid code search using BM25 lexical ranking and multi-signal reranking with 1-hop graph neighborhood expansion.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Natural language or symbol query to search in code"
                    },
                    "limit": {
                        "type": "integer",
                        "default": 5,
                        "description": "Maximum number of chunks to return"
                    },
                    "expand_graph": {
                        "type": "boolean",
                        "default": true,
                        "description": "Whether to attach 1-hop knowledge graph neighbors"
                    },
                    "fast": {
                        "type": "boolean",
                        "default": false,
                        "description": "Enable fast sub-millisecond lexical scoring without graph expansion"
                    }
                },
                "required": ["query"]
            }
        },
        {
            "name": "codeatlas_symbol_graph",
            "description": "Inspect callers, callees, and connected context for a specific code symbol.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "symbol": {
                        "type": "string",
                        "description": "Symbol name or node identifier to inspect"
                    },
                    "depth": {
                        "type": "integer",
                        "default": 1,
                        "description": "Neighborhood expansion depth"
                    }
                },
                "required": ["symbol"]
            }
        },
        {
            "name": "codeatlas_impact_analysis",
            "description": "Analyze change blast radius using reverse BFS to discover all symbols that would break if a given symbol is modified.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "symbol": {
                        "type": "string",
                        "description": "Symbol name or node ID to analyze for change blast radius"
                    }
                },
                "required": ["symbol"]
            }
        },
        {
            "name": "codeatlas_status",
            "description": "Get CodeAtlas index statistics (file count, symbol count, edges, chunks).",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }
    ])
}

fn send_response(id: &Value, result: Value) {
    let resp = json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    });
    let s = resp.to_string();
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let _ = writeln!(handle, "{}", s);
    let _ = handle.flush();
}

fn send_error(id: &Value, code: i64, message: &str) {
    let resp = json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    });
    let s = resp.to_string();
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let _ = writeln!(handle, "{}", s);
    let _ = handle.flush();
}

fn main() -> Result<()> {
    // MCP stdio protection: redirect ALL logging to stderr.
    // The MCP protocol owns stdout exclusively for JSON-RPC messages.
    // Any tracing/log output to stdout would corrupt the protocol stream.
    use tracing_subscriber::fmt;
    fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "warn".to_string())
                .as_str(),
        )
        .init();

    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let db_path = current_dir.join(".codeatlas").join("index.db");

    let engine = Engine::open(&current_dir, &db_path).ok();

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let request: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                send_error(&Value::Null, -32700, &format!("Parse error: {}", e));
                continue;
            }
        };

        let req_id = request.get("id").unwrap_or(&Value::Null);
        let method = match request.get("method").and_then(|m| m.as_str()) {
            Some(m) => m,
            None => {
                send_error(req_id, -32600, "Invalid Request");
                continue;
            }
        };

        let params = request.get("params").cloned().unwrap_or(json!({}));

        match method {
            "initialize" => {
                send_response(
                    req_id,
                    json!({
                        "protocolVersion": PROTOCOL_VERSION,
                        "capabilities": { "tools": {} },
                        "serverInfo": {
                            "name": SERVER_NAME,
                            "version": SERVER_VERSION
                        }
                    }),
                );
            }
            "notifications/initialized" => {
                // Client initialized notification, no response required
            }
            "ping" => {
                send_response(req_id, json!({}));
            }
            "tools/list" => {
                send_response(req_id, json!({ "tools": get_tools_def() }));
            }
            "tools/call" => {
                handle_tool_call(req_id, &params, engine.as_ref());
            }
            _ => {
                send_error(req_id, -32601, &format!("Method not found: {}", method));
            }
        }
    }

    Ok(())
}

fn handle_tool_call(req_id: &Value, params: &Value, engine: Option<&Engine>) {
    let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    let engine = match engine {
        Some(e) => e,
        None => {
            send_response(
                req_id,
                json!({
                    "isError": true,
                    "content": [{
                        "type": "text",
                        "text": "CodeAtlas database not initialized. Run `codeatlas index` first."
                    }]
                }),
            );
            return;
        }
    };

    let result = match tool_name {
        "codeatlas_search" => {
            let query = args.get("query").and_then(|q| q.as_str()).unwrap_or("");
            let limit = args.get("limit").and_then(|l| l.as_u64()).unwrap_or(5) as usize;
            let expand = args.get("expand_graph").and_then(|e| e.as_bool()).unwrap_or(true);
            let fast = args.get("fast").and_then(|f| f.as_bool()).unwrap_or(false);

            match engine.search_with_options(query, limit, expand, fast) {
                Ok(results) => json!(results),
                Err(e) => json!({ "error": e.to_string() }),
            }
        }
        "codeatlas_symbol_graph" => {
            let symbol = args.get("symbol").and_then(|s| s.as_str()).unwrap_or("");
            let depth = args.get("depth").and_then(|d| d.as_u64()).unwrap_or(1) as usize;

            let neighbors = engine.graph.expand_neighborhood(&[symbol.to_string()], depth);
            json!({
                "symbol": symbol,
                "neighbors_count": neighbors.len(),
                "neighbors": neighbors
            })
        }
        "codeatlas_impact_analysis" => {
            let symbol = args.get("symbol").and_then(|s| s.as_str()).unwrap_or("");
            let affected = engine.impact(symbol);
            json!({
                "target_symbol": symbol,
                "blast_radius_count": affected.len(),
                "callers_and_dependents": affected
            })
        }
        "codeatlas_status" => match engine.db.get_stats() {
            Ok(stats) => stats,
            Err(e) => json!({ "error": e.to_string() }),
        },
        _ => {
            send_error(req_id, -32602, &format!("Unknown tool: {}", tool_name));
            return;
        }
    };

    send_response(
        req_id,
        json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&result).unwrap_or_default()
            }]
        }),
    );
}
