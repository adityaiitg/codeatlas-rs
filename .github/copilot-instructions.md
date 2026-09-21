# GitHub Copilot Instructions for CodeAtlas

When exploring code, finding functions or classes, and planning modifications, use the `codeatlas` CLI:

## 1. Code Search & Navigation
- Prefer `codeatlas search "<query>"` over `grep` or `find`.
- To inspect callers, callees, and imports alongside search results, use:
  ```bash
  codeatlas search "<query>" --expand-graph
  ```

## 2. Refactoring & Blast Radius Analysis
- Always check what callers will break before modifying or removing an existing symbol:
  ```bash
  codeatlas impact "<symbol_name>"
  ```
- Update all reported callers and dependents to prevent regressions.

## 3. Codebase Indexing
- Index the workspace using `codeatlas index .` (takes ~100ms cold, ~6ms incremental).

## 4. Architecture Specifications
- To inspect or generate repository architecture diagrams:
  ```bash
  codeatlas wiki .codeatlas/wiki
  ```
