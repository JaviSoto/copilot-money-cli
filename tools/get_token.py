#!/usr/bin/env python3
from __future__ import annotations

import argparse
import sys
from pathlib import Path

from playwright.sync_api import sync_playwright


def load_creds(path: Path) -> tuple[str, str]:
    email = None
    password = None
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if "=" not in line:
            continue
        k, v = line.split("=", 1)
        k = k.strip().lower()
        v = v.strip()
        if k == "email":
            email = v
        elif k == "password":
            password = v
    if not email or not password:
        raise SystemExit(f"Missing email/password in {path}")
    return email, password


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Log into Copilot Money and print the API bearer token (stdout)."
    )
    parser.add_argument(
        "--secrets-file",
        default=str(Path("~/.codex/secrets/copilot_money").expanduser()),
        help="Path to secrets file containing email=... and password=...",
    )
    args = parser.parse_args()

    email, password = load_creds(Path(args.secrets_file).expanduser())

    token: str | None = None

    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        page = browser.new_page(viewport={"width": 1280, "height": 720})

        def on_request(req) -> None:
            nonlocal token
            if token is not None:
                return
            if req.url != "https://app.copilot.money/api/graphql":
                return
            auth = req.headers.get("authorization")
            if not auth:
                return
            # Expect "Bearer <jwt>"
            parts = auth.split(" ", 1)
            if len(parts) == 2 and parts[0].lower() == "bearer":
                token = parts[1].strip()

        page.on("request", on_request)

        page.goto("https://app.copilot.money/", wait_until="domcontentloaded", timeout=60_000)
        page.get_by_role("button", name="Continue with email").click()
        page.get_by_placeholder("Email address").fill(email)
        page.get_by_role("button", name="Continue", exact=True).click()

        page.get_by_role("button", name="Sign in with password instead").click()
        page.locator('input[type="password"]').first.fill(password)
        for name in ["Sign in", "Continue", "Log in"]:
            btn = page.get_by_role("button", name=name)
            if btn.count() > 0:
                btn.first.click()
                break

        page.wait_for_load_state("domcontentloaded", timeout=60_000)
        page.wait_for_timeout(4000)

        browser.close()

    if not token:
        print("failed to capture token", file=sys.stderr)
        return 1

    # Print token only; no newline surprises.
    sys.stdout.write(token)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

