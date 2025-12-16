# CLI Commands (Draft)

This is the initial command surface we’re aiming for. By default, commands are **read-only**; any write action requires `--apply`.

## Auth

- `copilot auth status` — show whether an auth token is configured and whether it works (no secret output).
- `copilot auth login` — obtain a token (initially via browser automation; later, fully API-based if possible).
- `copilot auth logout` — remove local token.

## Transactions

- `copilot transactions list` — list recent transactions.
  - Options: `--from`, `--to`, `--account`, `--reviewed/--unreviewed`, `--limit`
  - Output: `--output json|table`
- `copilot transactions search <query>` — search by merchant/name.
- `copilot transactions show <id>` — show a transaction with full details.
- `copilot transactions review <id...>` — mark reviewed (requires `--apply`).
- `copilot transactions unreview <id...>` — mark unreviewed (requires `--apply`).
- `copilot transactions set-category <id...> <category-id>` — set category (requires `--apply`).
- `copilot transactions assign-recurring <id...> <recurring-id>` — attach to an existing recurring (requires `--apply`).

## Categories

- `copilot categories list` — list categories (optionally include budgets/spend fields).
- `copilot categories show <id>` — show one category.

## Recurring

- `copilot recurrings list` — list recurring definitions.
- `copilot recurrings create` — create a recurring rule (requires `--apply`).
- `copilot recurrings show <id>` — show one recurring.

## Budgets

- `copilot budgets month <YYYY-MM>` — show month budget summary.
- `copilot budgets set <category-id> <amount>` — set category budget amount (requires `--apply`).

## Safety / Undo

- `copilot undo` — revert the last applied change made by this CLI (best-effort; stores a local journal).
- Global flags:
  - `--apply` to perform mutations
  - `--dry-run` to print the mutation without sending
  - `--output json|table`

