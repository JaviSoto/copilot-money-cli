# Security Policy

## Reporting

If you believe you’ve found a security issue **in this repo** (including potential credential exposure), please open a private report or contact the maintainer directly.

Please do **not** use this repo to report vulnerabilities in Copilot Money itself; report those to Copilot Money through their official channels.

## Credential hygiene

- Never commit API keys, tokens, cookies, passwords, or copied request headers.
- Captured network exports (HAR/Proxyman/etc) must stay outside the repo.
- Local secrets should live outside the repo (e.g., in `~/.codex/secrets/`).
- CI runs `gitleaks` to catch accidental secret commits.
