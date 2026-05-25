#!/usr/bin/env bash
# Local verifier — there is no CI.
# Usage: ./scripts/quality-gate.sh
# Strict: QUALITY_GATE_STRICT_CLEAN=1 ./scripts/quality-gate.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

step() {
  echo "→ $1"
  shift
  "$@"
  echo
}

echo "quality gate"
echo

step "cargo fmt --check" cargo fmt --check
step "cargo check" env RUSTFLAGS="-D warnings" cargo check --all-targets
step "cargo clippy" cargo clippy --all-targets -- -D warnings
step "cargo test" cargo test

echo "→ forbidden markers in src/"
if rg -n '\b(todo!|unimplemented!)\s*\(|\bunsafe\b' src --glob '*.rs' 2>/dev/null; then
  echo "failed: todo!/unimplemented!/unsafe in production source"
  exit 1
fi
echo

if [[ -f Cargo.toml ]]; then
  echo "→ zero external dependencies"
  count="$(cargo metadata --no-deps --format-version 1 \
    | python3 -c 'import json,sys; print(len(json.load(sys.stdin)["packages"]))')"
  if [[ "$count" != "1" ]]; then
    echo "failed: expected 1 package, got $count"
    exit 1
  fi
  echo
fi

if [[ "${QUALITY_GATE_STRICT_CLEAN:-0}" == "1" ]]; then
  echo "→ clean git working tree"
  if [[ -n "$(git status --porcelain)" ]]; then
    git status --short
    echo "failed: uncommitted changes"
    exit 1
  fi
  echo
fi

echo "quality gate passed"
