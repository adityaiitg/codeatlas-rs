---
name: codeatlas
description: Ultra-fast code intelligence and knowledge graph CLI engine (codeatlas). Use when the agent needs to search a codebase, find where functions or classes are defined, inspect call hierarchies, analyze the change blast radius before modifying code, or understand repository architecture. Triggers include "search code", "find function", "who calls this", "what breaks if I change this", "blast radius", "codebase architecture", "generate wiki", or exploring an unfamiliar project. Prefer codeatlas over grep/find for semantic and symbol-level code queries.
allowed-tools: Bash(codeatlas:*)
---

# codeatlas

High-performance codebase intelligence and deterministic knowledge graph CLI engine. Written in Rust for instant indexing (~100ms cold, ~6ms incremental), sub-millisecond multi-signal hybrid retrieval, and change impact blast-radius analysis.

Installation check:
```bash
which codeatlas || cargo install codeatlas
```

---

## ⚡ When to Use `codeatlas` vs Standard Tools

| Agent Task | Traditional Tool | Why `codeatlas` is Superior |
| :--- | :--- | :--- |
| **Locating a function/class** | `grep -rn "def foo"` | Ranks exact symbol definition **first**; filters out test fixtures, duplicates, and compatibility shims. |
| **Exploring unfamiliar project** | Reading files one by one | Indexes entire repo in **~100ms**; `codeatlas wiki` generates Mermaid dependency diagrams automatically. |
| **Refactoring or modifying code** | Guessing / grep callers | `codeatlas impact "<symbol>"` runs reverse BFS over the call graph and reveals **all breaking dependents** instantly. |
| **Context gathering for prompt** | Dumping whole files | Compact 40-line chunking saves 70% token budget with **0% context truncation**. |

---

## 🔁 Core Agent Workflows

### 1. Ensure the Codebase is Indexed
Before searching or analyzing, ensure the project index is initialized or up to date. Indexing is practically instant:
```bash
# Index current directory into .codeatlas/index.db (~100ms cold, ~6ms incremental)
codeatlas index .

# Force full re-index if files changed outside git
codeatlas index . --full
```

### 2. Multi-Signal Code Search
Find definitions, classes, methods, or concepts across the codebase.
```bash
# Standard search (returns ranked definitions and code snippets)
codeatlas search "<query>"

# Search with 1-hop knowledge graph neighborhood (callers, callees, and imports attached)
codeatlas search "<query>" --expand-graph

# Get machine-readable JSON (ideal for script parsing)
codeatlas search "<query>" --json
```

**Search Output Format**:
Each result reports the file path, line numbers, score, symbol identity, and snippet preview:
```
1. src/flask/templating.py 136:148 (score: 0.0484) [definition]
   Symbol: src/flask/templating.py:render_template
   │ def render_template(template_name_or_list, **context):
   └ 1-hop: _render (function), Environment (class)
```

### 3. Pre-Refactor Change Impact Analysis (CRITICAL)
**Always run this before renaming, modifying parameters, or deleting a function, method, or class.**
`codeatlas impact` performs reverse BFS over the call graph to discover every caller or dependent that relies on this symbol:
```bash
codeatlas impact "<symbol_name>"

# Example:
codeatlas impact "render_template"
# Output:
# Reverse Impact Analysis for "render_template" (2 callers/dependents):
#   ← src/flask/__init__.py
#   ← src/flask/views.py:View.dispatch_request
```
If dependents are detected, you **must** update those call sites as part of your refactoring plan.

### 4. Living Architecture Wiki & Diagrams
Synthesize architecture specifications with Mermaid dependency graphs and workflow sequence diagrams:
```bash
# Generate architecture wiki in .codeatlas/wiki/
codeatlas wiki .codeatlas/wiki

# View generated files:
# - .codeatlas/wiki/architecture.md (Mermaid subsystem dependency flowchart)
# - .codeatlas/wiki/workflows.md (Mermaid sequence diagrams of call traces)
# - .codeatlas/wiki/modules/*.md (Per-module classes, functions, and lines)
# - .codeatlas/wiki/index.md (Table of contents)
```

### 5. Knowledge Graph Inspection
```bash
# View repo graph statistics (counts of files, symbols, chunks, edges)
codeatlas graph stats

# Export graph for visualization in Graphviz or Gephi
codeatlas graph export --format dot -o graph.dot
codeatlas graph export --format json -o graph.json
```

---

## 💡 Best Practices for AI Agents

1. **Search First**: Before opening random files with `view_file` or running expensive `grep` commands across thousands of files, run `codeatlas search "<query>"`. It isolates the target definition in milliseconds.
2. **Blast Radius Check**: Before modifying an exported symbol or API signature, run `codeatlas impact "<symbol>"`. Do not make assumptions about which files import or call it.
3. **Expand Graph for Context**: When debugging a bug in a function, run `codeatlas search "<func>" --expand-graph`. The 1-hop graph neighbors will tell you which helper functions it invokes and which classes define it.
