#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fixture="${1:-fixtures/perf/filter_sum.wrela}"

if [[ "$(uname -m)" != "arm64" ]]; then
  echo "generated-code execution is not supported on this host" >&2
  exit 2
fi

cargo run -- perf compare --repeat 7 --json "$fixture"
