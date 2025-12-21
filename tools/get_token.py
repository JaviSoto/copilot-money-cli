#!/usr/bin/env python3
from __future__ import annotations

import argparse
import getpass
import sys
import time
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
        "--mode",
        choices=["interactive", "email-link", "credentials", "session"],
        default="interactive",
        help="Login flow: interactive (default), email-link (SSH-friendly), or credentials (uses secrets file).",
    )
    parser.add_argument(
        "--secrets-file",
        default=str(Path("~/.codex/secrets/copilot_money").expanduser()),
        help="Path to secrets file containing email=... and password=...",
    )
    parser.add_argument(
        "--email",
        help="Email address (required for --mode=email-link unless it can be inferred from --secrets-file).",
    )
    parser.add_argument(
        "--headful",
        action="store_true",
        help="Run browser headful (implied by --mode=interactive).",
    )
    parser.add_argument(
        "--user-data-dir",
        help="Optional Playwright Chromium user-data-dir for session persistence (sensitive).",
    )
    parser.add_argument(
        "--timeout-seconds",
        type=int,
        default=180,
        help="How long to wait for a GraphQL request with an Authorization token.",
    )
    args = parser.parse_args()

    mode = str(args.mode)
    interactive = mode == "interactive"
    email_link = mode == "email-link"
    credentials_mode = mode == "credentials"
    session_mode = mode == "session"
    headful = bool(args.headful) or interactive

    email = password = None
    if credentials_mode:
        email, password = load_creds(Path(args.secrets_file).expanduser())
    elif email_link:
        if args.email:
            email = args.email.strip()
        else:
            # Try to infer from secrets file (without requiring password use).
            try:
                inferred_email, _ = load_creds(Path(args.secrets_file).expanduser())
                email = inferred_email
            except Exception:
                email = None
        if not email:
            print("--email is required for --mode=email-link", file=sys.stderr)
            return 2

    token: str | None = None

    with sync_playwright() as p:
        if args.user_data_dir:
            context = p.chromium.launch_persistent_context(
                args.user_data_dir,
                headless=not headful,
                viewport={"width": 1280, "height": 720},
            )
            page = context.new_page()
        else:
            browser = p.chromium.launch(headless=not headful)
            page = browser.new_page(viewport={"width": 1280, "height": 720})

        def on_request(req) -> None:
            nonlocal token
            if token is not None:
                return
            if not req.url.startswith("https://app.copilot.money/api/graphql"):
                return
            auth = req.headers.get("authorization")
            if not auth:
                return
            # Expect "Bearer <jwt>"
            parts = auth.split(" ", 1)
            if len(parts) == 2 and parts[0].lower() == "bearer":
                token = parts[1].strip()

        page.on("request", on_request)

        def click_continue_with_email() -> None:
            # Copilot shows a landing screen before the email input appears.
            # Be resilient to minor UI changes and headless rendering differences.
            locators = [
                page.get_by_role("button", name="Continue with email"),
                page.locator('button:has-text("Continue with email")'),
                page.locator('text=Continue with email'),
            ]
            for _ in range(40):
                for loc in locators:
                    try:
                        if loc.count() > 0:
                            loc.first.click(force=True)
                            return
                    except Exception:
                        pass
                page.wait_for_timeout(250)

        def fill_email_address(addr: str) -> None:
            selectors = [
                page.get_by_placeholder("Email address"),
                page.locator('input[type="email"]'),
                page.locator('input[name="email"]'),
                page.locator('input[autocomplete="email"]'),
            ]
            for _ in range(40):
                for sel in selectors:
                    try:
                        if sel.count() > 0:
                            sel.first.click()
                            sel.first.fill(addr)
                            return
                    except Exception:
                        pass
                page.wait_for_timeout(250)
            raise SystemExit('could not find email input')

        def click_continue() -> None:
            for name in ["Continue", "Send link", "Next"]:
                try:
                    btn = page.get_by_role("button", name=name, exact=False)
                    if btn.count() > 0 and btn.first.is_enabled():
                        btn.first.click()
                        return
                except Exception:
                    pass
            # last resort: first button
            try:
                page.locator('button').first.click()
                return
            except Exception:
                raise SystemExit('could not click Continue')

        url = "https://app.copilot.money/"
        if email_link or credentials_mode:
            url = "https://app.copilot.money/login"
        page.goto(url, wait_until="domcontentloaded", timeout=60_000)
        if session_mode:
            # Rely on the persisted browser session to already be logged in.
            # If no session exists, the caller should run interactive login once with `--user-data-dir`.
            pass
        elif interactive:
            print(
                "Waiting for you to log in in the opened browser window...",
                file=sys.stderr,
            )
        elif email_link:
            try:
                click_continue_with_email()
            except Exception:
                pass
            page.wait_for_timeout(250)
            fill_email_address(email)
            click_continue()

            # User must paste the magic link from their email. Hide input because it may contain tokens.
            link = getpass.getpass(
                "Paste Copilot sign-in link URL from your email (input hidden): "
            ).strip()
            if not link.startswith("http"):
                print("invalid link", file=sys.stderr)
                browser.close()
                return 2
            page.goto(link, wait_until="domcontentloaded", timeout=60_000)
        else:
            try:
                click_continue_with_email()
            except Exception:
                pass
            page.wait_for_timeout(250)
            fill_email_address(email)
            click_continue()

            page.get_by_role("button", name="Sign in with password instead").click()
            page.locator('input[type="password"]').first.fill(password)
            for name in ["Sign in", "Continue", "Log in"]:
                btn = page.get_by_role("button", name=name)
                if btn.count() > 0:
                    btn.first.click()
                    break

        deadline = time.time() + max(1, int(args.timeout_seconds))
        while token is None and time.time() < deadline:
            page.wait_for_timeout(250)

        if args.user_data_dir:
            page.context.close()
        else:
            browser.close()

    if not token:
        if session_mode and args.user_data_dir:
            print(
                "failed to capture token using persisted session; run `copilot auth login --persist-session` once",
                file=sys.stderr,
            )
        else:
            print("failed to capture token", file=sys.stderr)
        return 1

    # Print token only; no newline surprises.
    sys.stdout.write(token)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
