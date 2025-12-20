#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

git config core.hooksPath .githooks
echo "Configured git hooks path to .githooks"

if ! command -v gitleaks >/dev/null 2>&1; then
  echo "Note: gitleaks is not installed; CI will run it, but installing locally is recommended."
fi

