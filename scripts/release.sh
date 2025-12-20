#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

usage() {
  cat <<'EOF'
Usage: scripts/release.sh <version>

Example:
  scripts/release.sh 0.1.0

What it does:
- Runs fmt/test/clippy
- Updates Cargo.toml version + CHANGELOG
- Commits + tags v<version>
- Pushes to origin (triggers GitHub Release build)
- Publishes to crates.io (requires `cargo login`)
EOF
}

if [ "${1:-}" = "" ] || [ "${1:-}" = "-h" ] || [ "${1:-}" = "--help" ]; then
  usage
  exit 2
fi

VERSION="$1"
TAG="v$VERSION"
DATE="$(date +%Y-%m-%d)"

if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "error: working tree not clean" >&2
  exit 1
fi

BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [ "$BRANCH" != "main" ]; then
  echo "error: must be on main (current: $BRANCH)" >&2
  exit 1
fi

cargo fmt --all -- --check
cargo test
cargo clippy -- -D warnings

python3 - <<PY
from pathlib import Path
import re

version = "$VERSION"
date = "$DATE"

# Bump Cargo.toml
p = Path('Cargo.toml')
text = p.read_text(encoding='utf-8')
text = re.sub(r'^version\s*=\s*"[^"]+"\s*$', f'version = "{version}"', text, flags=re.M)
p.write_text(text, encoding='utf-8')

# Ensure CHANGELOG has entry
def ensure_changelog_entry() -> None:
    ch = Path('CHANGELOG.md')
    if not ch.exists():
        ch.write_text('# Changelog\n\n', encoding='utf-8')

    ch_text = ch.read_text(encoding='utf-8')
    header = f"## [{version}] - {date}\n\n- TODO\n\n"

    if f"## [{version}]" in ch_text:
        return

    # Insert after the first blank line after the title.
    lines = ch_text.splitlines(keepends=True)
    out = []
    inserted = False
    blank_seen = False
    for line in lines:
        out.append(line)
        if not inserted:
            if line.strip() == '':
                if blank_seen:
                    out.append(header)
                    inserted = True
                blank_seen = True
            else:
                blank_seen = False

    if not inserted:
        out.append('\n')
        out.append(header)

    ch.write_text(''.join(out), encoding='utf-8')

ensure_changelog_entry()
PY

cargo build --locked

git add Cargo.toml CHANGELOG.md Cargo.lock || true

git commit -m "Release $VERSION"

git tag "$TAG"

git push origin main

git push origin "$TAG"

cargo publish

echo "ok: pushed $TAG, published $VERSION"
