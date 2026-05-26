#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

cargo run -- perf compile --mode dev --repeat 1 --json fixtures/mir/data_flow.wrela
cargo run -- perf code --mode dev --repeat 1 --json fixtures/perf/scalar_const.wrela
