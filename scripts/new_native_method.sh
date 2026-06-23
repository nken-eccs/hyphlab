#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

cargo run -p hyph-cli -- dev new-adapter "$@"
