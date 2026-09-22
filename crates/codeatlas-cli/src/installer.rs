use std::fs;
use std::path::PathBuf;
use anyhow::Result;
use colored::Colorize;
use serde_json::{json, Value};

pub struct InstallResult {
    pub agent: String,
    pub config_path: PathBuf,
    pub action: String, // "configured", "already_present", "removed", "skipped"
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn get_agent_paths(agent: &str) -> Vec<PathBuf> {
    let home = home_dir().unwrap_or_else(|| PathBuf::from("."));
    match agent {
        "claude" => vec![home.join(".claude.json")],
        "cursor" => vec![home.join(".cursor").join("mcp.json"), PathBuf::from(".cursor/mcp.json")],
        "opencode" => vec![home.join(".config").join("opencode").join("config.json")],
        "codex" => vec![home.join(".codex").join("config.json")],
        _ => vec![],
    }
}

pub fn configure_agent(agent: &str, remove: bool) -> Result<Vec<InstallResult>> {
    let mut results = Vec::new();
    let paths = get_agent_paths(agent);

    let current_exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("codeatlas"));
    let mcp_bin = current_exe
        .parent()
        .map(|p| p.join("codeatlas-mcp"))
        .unwrap_or_else(|| PathBuf::from("codeatlas-mcp"));

    for config_file in paths {
        if !remove {
            if let Some(parent) = config_file.parent() {
                let _ = fs::create_dir_all(parent);
            }
        } else if !config_file.exists() {
            continue;
        }

        let mut data: Value = if config_file.exists() {
            let content = fs::read_to_string(&config_file).unwrap_or_default();
            // Strip JSONC-style line/block comments before parsing (Cursor & VS Code use JSONC)
            let stripped = strip_jsonc_comments(&content);
            // Create .bak backup before mutating any existing config
            let bak_path = config_file.with_extension("json.bak");
            let _ = fs::copy(&config_file, &bak_path);
            match serde_json::from_str(&stripped) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!(
                        "{} Could not parse {} ({}). Skipping to avoid corruption.",
                        "Warning:".yellow().bold(),
                        config_file.display(),
                        e
                    );
                    results.push(InstallResult {
                        agent: agent.to_string(),
                        config_path: config_file,
                        action: "skipped".to_string(),
                    });
                    continue;
                }
            }
        } else {
            json!({})
        };

        if remove {
            let mut modified = false;
            if agent == "opencode" {
                if let Some(mcp) = data.get_mut("mcp").and_then(|v| v.as_object_mut()) {
                    if mcp.remove("codeatlas").is_some() {
                        modified = true;
                    }
                }
            } else if let Some(mcp) = data.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
                if mcp.remove("codeatlas").is_some() {
                    modified = true;
                }
            }

            if modified {
                fs::write(&config_file, serde_json::to_string_pretty(&data)?)?;
                results.push(InstallResult {
                    agent: agent.to_string(),
                    config_path: config_file,
                    action: "removed".to_string(),
                });
            }
        } else {
            let entry = if agent == "opencode" {
                json!({
                    "type": "local",
                    "command": [mcp_bin.to_string_lossy().to_string()],
                    "enabled": true
                })
            } else {
                json!({
                    "command": mcp_bin.to_string_lossy().to_string(),
                    "args": [],
                    "type": "stdio"
                })
            };

            let key = if agent == "opencode" { "mcp" } else { "mcpServers" };
            if !data.is_object() {
                data = json!({});
            }

            let obj = data.as_object_mut().unwrap();
            let servers = obj.entry(key).or_insert_with(|| json!({}));
            if let Some(servers_obj) = servers.as_object_mut() {
                if servers_obj.contains_key("codeatlas") {
                    results.push(InstallResult {
                        agent: agent.to_string(),
                        config_path: config_file,
                        action: "already_present".to_string(),
                    });
                    continue;
                }
                servers_obj.insert("codeatlas".to_string(), entry);
            }

            fs::write(&config_file, serde_json::to_string_pretty(&data)?)?;
            results.push(InstallResult {
                agent: agent.to_string(),
                config_path: config_file,
                action: "configured".to_string(),
            });
        }
    }

    Ok(results)
}

pub fn print_install_results(results: &[InstallResult]) {
    for r in results {
        match r.action.as_str() {
            "configured" => {
                println!(
                    "{} Configured CodeAtlas MCP in {} at {}",
                    "✓".green().bold(),
                    r.agent.bold().cyan(),
                    r.config_path.display().to_string().dimmed()
                );
            }
            "already_present" => {
                println!(
                    "{} Already configured in {} at {}",
                    "ℹ".blue().bold(),
                    r.agent.bold(),
                    r.config_path.display().to_string().dimmed()
                );
            }
            "removed" => {
                println!(
                    "{} Removed CodeAtlas MCP from {} at {}",
                    "✓".yellow().bold(),
                    r.agent.bold(),
                    r.config_path.display().to_string().dimmed()
                );
            }
            "skipped" => {
                eprintln!(
                    "{} Skipped {} (config parse failed — check backup .json.bak)",
                    "!".red().bold(),
                    r.config_path.display()
                );
            }
            _ => {}
        }
    }
}

/// Strip line comments (`// ...`) and block comments (`/* ... */`) from a JSON string.
/// This enables safe parsing of JSONC files used by Cursor, VS Code, and similar editors.
pub fn strip_jsonc_comments(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    let mut in_string = false;
    let mut prev_char = '\0';

    while let Some(c) = chars.next() {
        if in_string {
            if c == '"' && prev_char != '\\' {
                in_string = false;
            }
            result.push(c);
            prev_char = c;
        } else if c == '"' {
            in_string = true;
            result.push(c);
            prev_char = c;
        } else if c == '/' && chars.peek() == Some(&'/') {
            // Line comment: skip until newline
            for cc in chars.by_ref() {
                if cc == '\n' {
                    result.push('\n');
                    break;
                }
            }
            prev_char = '\n';
        } else if c == '/' && chars.peek() == Some(&'*') {
            // Block comment: skip until */
            chars.next(); // consume '*'
            let mut last = '\0';
            for cc in chars.by_ref() {
                if last == '*' && cc == '/' {
                    break;
                }
                if cc == '\n' {
                    result.push('\n');
                }
                last = cc;
            }
            prev_char = ' ';
        } else {
            result.push(c);
            prev_char = c;
        }
    }
    result
}
