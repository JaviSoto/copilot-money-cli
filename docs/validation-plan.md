# Validation Plan (Before Automation)

Goal: prove we can perform key Copilot actions reliably and reversibly before building “auto-triage”.

## Principles

- **Small scope:** start with 3–10 known transactions.
- **Deterministic IDs:** identify each transaction by *(date, merchant, amount, account)* and (when available) Copilot’s internal transaction ID.
- **One action at a time:** perform a single mutation, verify, then proceed.
- **Always reversible:** prefer flows with Copilot “Undo” when available; otherwise record prior values so we can restore them.
- **Default dry-run:** any future CLI “write” command should require `--apply`.

## Suggested test cases (manual selection by Javi)

Pick a short list you don’t mind us touching:

1. A simple card purchase (easy to categorize)
2. A transfer (often tricky / needs correct category)
3. A known recurring item (or one you want to turn into recurring)

For each, provide: date, merchant, amount, and the account name shown in Copilot.

## Actions to validate

1. Mark as reviewed / unreviewed
2. Search transactions by merchant and date range
3. Bulk-select and mark reviewed
4. Change category
5. Assign to recurring
6. Create recurring definition
7. Undo / revert

## Evidence / audit trail

For each action we take, we’ll log:

- timestamp (local)
- operation (what we attempted)
- transaction identifier(s)
- before/after screenshots (UI flow) **or** mutation request + response summaries (API flow)

