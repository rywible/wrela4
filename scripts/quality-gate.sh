#!/usr/bin/env bash
# Canonical local quality gate. There is no CI — run this before claiming done.
# Usage: scripts/quality-gate.sh [repo-root]
set -euo pipefail

ROOT="${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
cd "$ROOT"

echo "=== quality gate: $ROOT ==="
echo ""

echo "--- cargo fmt --check ---"
cargo fmt --check
echo "PASS"
echo ""

echo "--- RUSTFLAGS=-D warnings cargo check --all-targets ---"
RUSTFLAGS="-D warnings" cargo check --all-targets
echo "PASS"
echo ""

echo "--- cargo clippy --all-targets -- -D warnings ---"
cargo clippy --all-targets -- -D warnings
echo "PASS"
echo ""

echo "--- cargo test ---"
cargo test
echo "PASS"
echo ""

echo "--- forbidden marker scan (src/) ---"
if rg -n '\b(todo!|unimplemented!)\s*\(|\bunsafe\b' src --glob '*.rs' 2>/dev/null; then
  echo "FAIL: forbidden markers in production source"
  exit 1
fi
echo "PASS (no matches)"
echo ""

if [[ -f Cargo.toml ]]; then
  echo "--- cargo metadata (deps) ---"
  pkg_count="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(len(json.load(sys.stdin)["packages"]))')"
  if [[ "$pkg_count" != "1" ]]; then
    echo "FAIL: expected 1 package (zero deps), got $pkg_count"
    exit 1
  fi
  echo "PASS (zero external dependencies)"
  echo ""
fi

if [[ "${QUALITY_GATE_STRICT_CLEAN:-0}" == "1" ]]; then
  echo "--- git status (strict clean) ---"
  if [[ -n "$(git status --porcelain)" ]]; then
    git status --short
    echo "FAIL: working tree not clean"
    exit 1
  fi
  echo "PASS"
  echo ""
fi

echo "=== quality gate passed ==="
