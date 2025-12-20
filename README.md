# copilot-money-cli

[![CI](https://github.com/JaviSoto/copilot-money-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/JaviSoto/copilot-money-cli/actions/workflows/ci.yml)
[![Release](https://github.com/JaviSoto/copilot-money-cli/actions/workflows/release.yml/badge.svg)](https://github.com/JaviSoto/copilot-money-cli/actions/workflows/release.yml)
[![Coverage](https://img.shields.io/badge/coverage-82%25-blue)](https://github.com/JaviSoto/copilot-money-cli/actions/workflows/ci.yml)

Unofficial CLI client for Copilot Money.

Vibe-coded with GPT 5.2 in Codex. Use with caution.

---

## Demo (mock data)

This demo runs the CLI against the repo’s fixture data (no network calls):

![copilot-money-cli demo](https://raw.githubusercontent.com/JaviSoto/copilot-money-cli/main/assets/demo.gif)

## Install

- Homebrew: `brew install JaviSoto/tap/copilot-money-cli`
- With Cargo: `cargo install copilot-money-cli` (installs the `copilot` binary)
- Or download a prebuilt binary from GitHub Releases

## Quick start

- `copilot auth status`
- `copilot auth login --mode email-link --email you@example.com`
- `copilot transactions list --unreviewed --fields date,name,amount,category`

## Auth

- `copilot auth login` tries to use an optional browser helper (Python + Playwright); otherwise it falls back to manual token paste.
- SSH-friendly: `copilot auth login --mode email-link --email you@example.com` (or just paste a bearer token manually).

## Command reference

By default, commands are **read-only**. Any write action either:

- runs with an interactive confirmation prompt, or
- requires `--yes` in non-interactive contexts (scripts/CI).

### Global flags

- `--dry-run` prints the planned change without sending it
- `--yes` skips confirmation prompts
- `--output json|table`
- `--color auto|always|never`

### Auth

- `copilot auth status` — show whether an auth token is configured and whether it works (no secret output).
- `copilot auth set-token` — securely store a token (prompts with hidden input).
- `copilot auth login` — obtain and store a token (uses optional Python+Playwright helper; otherwise prompts for manual token paste).
  - `--mode interactive` (default): opens a browser window and waits.
  - `--mode email-link`: SSH-friendly; you paste the sign-in link back (hidden input).
  - `--mode credentials`: uses `--secrets-file` (not recommended).
  - `--persist-session`: stores a Playwright browser session under `~/.config/copilot-money-cli/playwright-session` so tokens can be refreshed without re-auth.
- `copilot auth refresh` — refresh token from the persisted browser session.
- `copilot auth logout` — remove local token.

### Transactions

- `copilot transactions list` — list transactions (paged).
  - Pagination: `--limit`, `--after`, `--pages`, `--all`, `--page-info`
  - Filters: `--reviewed`, `--unreviewed`, `--category-id`, `--category <NAME>`, `--tag <TAG>` (repeatable), `--date <DATE>`, `--name-contains <TEXT>`
  - Sorting: `--sort date-desc|date-asc|amount-desc|amount-asc`
  - Table columns: `--fields date,name,amount,reviewed,category,tags,type,id`
- `copilot transactions search <query>` — list transactions and filter by merchant/name substring.
- `copilot transactions show <id>` — show a transaction with full details.
- `copilot transactions review <id...>` — mark reviewed.
- `copilot transactions unreview <id...>` — mark unreviewed.
- `copilot transactions set-category <id...> --category-id <ID>` — set category by id.
- `copilot transactions set-category <id...> --category <NAME>` — set category by name (exact match).
- `copilot transactions assign-recurring <id...> --recurring-id <ID>` — attach to an existing recurring.
- `copilot transactions set-notes <id...> --notes <TEXT>` — set notes.
- `copilot transactions set-notes <id...> --clear` — clear notes.
- `copilot transactions set-tags <id...> [--mode set|add|remove] [--tag-id <TAG_ID> ...]` — update tags.
- `copilot transactions edit <id...> --type <TYPE>` — set transaction type (best-effort).

### Categories

- `copilot categories list` — list categories.
  - Options: `--children`, `--name-contains`, `--spend`, `--budget`, `--rollovers`
- `copilot categories show <id>` — show one category.
- `copilot categories create <name> [--emoji <EMOJI>] [--color-name <COLOR>] [--excluded] [--template-id <ID>] [--budget-unassigned-amount <AMOUNT>]` — create a category.

### Recurring

- `copilot recurrings list` — list recurring definitions.
  - Options: `--category-id`, `--name-contains`
- `copilot recurrings create <transaction-id> --frequency <FREQ>` — create a recurring from a transaction (best-effort).
- `copilot recurrings edit <id> [--name-contains <TEXT>] [--min-amount <N>] [--max-amount <N>] [--recalculate-only-for-future]` — edit recurring rule (best-effort).
- `copilot recurrings show <id>` — show one recurring.

### Tags

- `copilot tags list` — list tags.
- `copilot tags create <name> [--color-name <COLOR>]` — create a tag.
- `copilot tags delete <id>` — delete a tag.

### Budgets

- `copilot budgets month` — list budget history months (best-effort).
- `copilot budgets set` — not implemented yet.

## Development

### Demo generation

Demos are generated from fixture responses under `tests/fixtures/graphql/`.

- Generate the GIF: `./scripts/generate-demos.sh`
- Tape file: `demo/basic.tape`

### Schema stub

- Generate/update schema stub: `cargo run --bin schema-gen -- --out schema/schema.graphql`

### Coverage

- Summary: `cargo llvm-cov --workspace --summary-only`
- Update README badge: `./scripts/update-coverage.sh`

## Open source

This is an **unofficial** project and is **not affiliated with Copilot Money**.

**Trademarks:** “Copilot Money” and related marks/logos are the property of their respective owners.

**Terms/Legal:** This tool interacts with Copilot Money’s web API. Use may be restricted by Copilot Money’s Terms of Service.
You are responsible for ensuring your use complies with applicable terms and laws.
