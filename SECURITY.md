# Security Policy

## Reporting

If you believe you’ve found a security issue (including potential credential exposure), please open a private report or contact the maintainer directly.

## Credential hygiene

- Never commit API keys, tokens, cookies, passwords, or copied request headers.
- Local secrets should live outside the repo (e.g., in `~/.codex/secrets/`).
- CI runs `gitleaks` to catch accidental secret commits.

