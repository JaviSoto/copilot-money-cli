#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

if ! cargo llvm-cov --version >/dev/null 2>&1; then
  echo "error: cargo-llvm-cov is not installed (run: cargo install cargo-llvm-cov)" >&2
  exit 1
fi

OUT=$(cargo llvm-cov --workspace --summary-only)
TOTAL=$(printf '%s\n' "$OUT" | rg '^TOTAL' | tail -n 1)

LINES_PCT=$(printf '%s\n' "$TOTAL" | rg -o '[0-9]+\.[0-9]+%' | tail -n 1)
LINES_PCT=${LINES_PCT%%%}

LINES_INT=$(python3 - <<PY
v=float('$LINES_PCT')
print(int(round(v)))
PY
)

BADGE="[![Coverage](https://img.shields.io/badge/coverage-${LINES_INT}%25-blue)](https://github.com/JaviSoto/copilot-money-cli/actions/workflows/ci.yml)"

python3 - <<PY
from pathlib import Path
badge = "$BADGE"
readme = Path('README.md')
text = readme.read_text(encoding='utf-8')
lines = text.splitlines()

out = []
replaced = False
for line in lines:
    if line.startswith('[![Coverage]('):
        out.append(badge)
        replaced = True
    else:
        out.append(line)

if not replaced:
    insert_at = None
    for i,l in enumerate(out):
        if l.startswith('[![CI]('):
            insert_at = i
            break
    if insert_at is not None:
        j = insert_at
        while j < len(out) and out[j].startswith('[!['):
            j += 1
        out.insert(j, badge)

readme.write_text('\n'.join(out) + ('\n' if text.endswith('\n') else ''), encoding='utf-8')
PY

echo "Updated README coverage badge to ${LINES_INT}% (lines)"
