#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

if ! command -v vhs >/dev/null 2>&1; then
  echo "error: vhs is required. Install it: brew install vhs" >&2
  exit 1
fi

cargo build --release --locked --bin copilot

mkdir -p assets

# Some environments lack a usable Chromium sandbox. VHS supports opting out.
export VHS_NO_SANDBOX=1

vhs demo/basic.tape -o assets/demo.gif

echo "Wrote assets/demo.gif"
