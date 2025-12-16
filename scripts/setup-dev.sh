#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

git config core.hooksPath .githooks
echo "Configured git hooks path to .githooks"

