#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

cargo bench -p hyph-adapters --bench crate_baselines
cargo bench -p hyph-adapters --features rust-hyphenation-embedded --bench crate_baselines
