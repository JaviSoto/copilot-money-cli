# copilot-money-api

Unofficial CLI + API client for Copilot Money.

## Goals

- Script common workflows (review, categorize, recurring rules) safely.
- Output in `json` and nicely formatted terminal tables.
- Prefer direct API calls (no browser) once the GraphQL surface is mapped.

## Safety

- Default to read-only (dry-run) behavior.
- Any write action should require an explicit `--apply`.
- Keep an `undo` path (either Copilot’s own undo, or replaying prior field values).

## Language

Rust (CLI + client). A couple of helper scripts live under `tools/`.

## Status

Scaffold only (no API calls yet).

## Dev notes

- Copilot’s web app uses GraphQL at `https://app.copilot.money/api/graphql`.
- Server-side introspection is disabled, so schema download requires alternative approaches (e.g., capturing operation documents from the web app traffic).

## Guardrails

- CI runs `gitleaks` to catch accidental credential commits.
- For local hooks: `./scripts/setup-dev.sh` (and install `gitleaks` for staged scanning).
