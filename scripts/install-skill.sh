#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "=== CodeAtlas CLI Skill Installer ==="

# 1. Install globally for Antigravity (AGY) and other agent environments
AGENTS_DIR="$HOME/.agents/skills/codeatlas"
GEMINI_DIR="$HOME/.gemini/config/skills"

mkdir -p "$AGENTS_DIR"
cp "$REPO_ROOT/skills/codeatlas/SKILL.md" "$AGENTS_DIR/SKILL.md"

if [ -d "$HOME/.gemini/config" ]; then
  mkdir -p "$GEMINI_DIR"
  ln -sfn "$AGENTS_DIR" "$GEMINI_DIR/codeatlas"
  echo "✓ Installed globally for Antigravity (AGY) at $GEMINI_DIR/codeatlas"
fi

echo "✓ Installed globally for agent environments at $AGENTS_DIR/SKILL.md"

# 2. Optionally install into a target project directory
TARGET_DIR="${1:-}"
if [ -n "$TARGET_DIR" ] && [ "$TARGET_DIR" != "$REPO_ROOT" ] && [ -d "$TARGET_DIR" ]; then
  # Cursor rule
  mkdir -p "$TARGET_DIR/.cursor/rules"
  cp "$REPO_ROOT/.cursor/rules/codeatlas.mdc" "$TARGET_DIR/.cursor/rules/codeatlas.mdc"
  echo "✓ Installed Cursor rule at $TARGET_DIR/.cursor/rules/codeatlas.mdc"

  # GitHub Copilot instructions
  mkdir -p "$TARGET_DIR/.github"
  cp "$REPO_ROOT/.github/copilot-instructions.md" "$TARGET_DIR/.github/copilot-instructions.md"
  echo "✓ Installed GitHub Copilot instructions at $TARGET_DIR/.github/copilot-instructions.md"
fi

echo ""
echo "CodeAtlas CLI skill is ready to use!"
