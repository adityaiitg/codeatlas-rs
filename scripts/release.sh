#!/usr/bin/env bash
set -euo pipefail

# Ensure we're in repo root
cd "$(dirname "$0")/.."

echo "=== CodeAtlas Rust Release Automation ==="

# Check git status
if [[ -n $(git status --porcelain) ]]; then
  echo "Error: Working directory has uncommitted changes. Commit or stash them first."
  exit 1
fi

echo "Running workspace tests..."
cargo test --workspace

echo ""
echo "Ready to publish to crates.io:"
echo "  1. codeatlas-core"
echo "  2. codeatlas (CLI)"
echo "  3. codeatlas-mcp"
echo ""
read -p "Do you want to proceed with crates.io publishing? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
  echo "Publishing codeatlas-core..."
  cargo publish -p codeatlas-core

  echo "Waiting 30 seconds for crates.io index propagation..."
  sleep 30

  echo "Publishing codeatlas CLI..."
  cargo publish -p codeatlas

  echo "Publishing codeatlas-mcp..."
  cargo publish -p codeatlas-mcp

  echo "✓ Successfully published all packages to crates.io!"
fi
