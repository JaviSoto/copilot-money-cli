#!/usr/bin/env python3
from __future__ import annotations

import argparse
import getpass
import html
import json
import os
import platform
import re
import shutil
import sys
import tempfile
import time
from base64 import urlsafe_b64decode
from pathlib import Path

from playwright.sync_api import sync_playwright


def trace(message: str) -> None:
    if os.environ.get("COPILOT_DEBUG_GET_TOKEN") == "1":
        print(f"[get_token] {message}", file=sys.stderr, flush=True)


def _reexec_into_integrations_venv_if_needed() -> None:
    if os.environ.get("CODEX_INTEGRATIONS_VENV_REEXEC") == "1":
        return
    try:
        import googleapiclient  # noqa: F401

        return
    except Exception:
        pass

    venv_py = Path.home() / ".codex" / "integrations" / "venv" / "bin" / "python"
    if not venv_py.exists():
        return
    os.environ["CODEX_INTEGRATIONS_VENV_REEXEC"] = "1"
    os.execv(str(venv_py), [str(venv_py), str(Path(__file__).resolve()), *sys.argv[1:]])


def load_secret_fields(path: Path) -> dict[str, str]:
    fields: dict[str, str] = {}
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        key = key.strip().lower()
        value = value.strip()
        if key and value:
            fields[key] = value
    return fields


def infer_email(explicit_email: str | None, secrets_file: Path) -> str | None:
    if explicit_email and explicit_email.strip():
        return explicit_email.strip()
    try:
        return load_secret_fields(secrets_file).get("email")
    except Exception:
        return None


def infer_password(secrets_file: Path) -> str | None:
    try:
        password = load_secret_fields(secrets_file).get("password")
    except Exception:
        return None
    return password.strip() if password and password.strip() else None


def _reexec_under_xvfb_if_needed(mode: str, headful: bool) -> None:
    if mode not in {"email-link", "credentials", "session"}:
        return
    if os.environ.get("COPILOT_XVFB_REEXEC") == "1":
        return
    if os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY"):
        return
    xvfb_run = shutil.which("xvfb-run")
    if not xvfb_run:
        return

    argv = [xvfb_run, "-a", sys.executable, str(Path(__file__).resolve()), *sys.argv[1:]]
    if not headful and "--headful" not in sys.argv[1:]:
        argv.append("--headful")
    os.environ["COPILOT_XVFB_REEXEC"] = "1"
    os.execvp(argv[0], argv)






def decode_jwt_payload(token: str) -> dict | None:
    parts = token.strip().split(".")
    if len(parts) < 2:
        return None
    payload = parts[1]
    payload += "=" * ((4 - (len(payload) % 4)) % 4)
    try:
        raw = urlsafe_b64decode(payload.encode("utf-8"))
        obj = json.loads(raw.decode("utf-8"))
    except Exception:
        return None
    return obj if isinstance(obj, dict) else None


def token_is_fresh(token: str, *, grace_seconds: int = 60) -> bool:
    payload = decode_jwt_payload(token)
    if not payload:
        return False
    exp = payload.get("exp")
    if not isinstance(exp, int | float):
        return False
    return float(exp) > (time.time() + float(grace_seconds))


def _cleanup_stale_singleton_artifacts(session_dir: Path) -> bool:
    removed_any = False
    for name in ("SingletonCookie", "SingletonLock", "SingletonSocket"):
        path = session_dir / name
        try:
            if path.is_symlink() or path.exists():
                path.unlink()
                removed_any = True
                trace(f"removed stale Chromium singleton artifact {path}")
        except FileNotFoundError:
            continue
    default_lock = session_dir / "Default" / "LOCK"
    try:
        if default_lock.exists():
            default_lock.unlink()
            removed_any = True
            trace(f"removed stale Chromium lock file {default_lock}")
    except FileNotFoundError:
        pass
    return removed_any


def preferred_browser_channel() -> str | None:
    explicit = os.environ.get("COPILOT_PLAYWRIGHT_CHANNEL", "").strip()
    if explicit:
        return explicit
    if platform.system() == "Darwin" and Path("/Applications/Google Chrome.app").exists():
        return "chrome"
    if shutil.which("google-chrome") or shutil.which("google-chrome-stable"):
        return "chrome"
    return None


def should_force_headful_for_login(mode: str, *, browser_channel: str | None) -> bool:
    if mode not in {"email-link", "credentials"}:
        return False
    if not browser_channel:
        return False
    if platform.system() == "Darwin":
        return True
    return bool(os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY"))


def launch_browser_context(playwright, *, user_data_dir: str | None, headful: bool):
    channel = preferred_browser_channel()

    def launch_kwargs() -> dict:
        kwargs = {
            "headless": not headful,
        }
        if channel:
            kwargs["channel"] = channel
        return kwargs

    def launch(dir_value: str | None):
        if dir_value:
            trace(f"launching persistent browser session dir={dir_value} channel={channel or 'bundled'}")
            return playwright.chromium.launch_persistent_context(
                dir_value,
                viewport={"width": 1280, "height": 720},
                **launch_kwargs(),
            )
        trace(f"launching ephemeral browser channel={channel or 'bundled'}")
        browser = playwright.chromium.launch(**launch_kwargs())
        return browser.new_context(viewport={"width": 1280, "height": 720})

    try:
        return launch(user_data_dir)
    except Exception as exc:
        if not user_data_dir:
            raise
        session_dir = Path(user_data_dir)
        if not session_dir.exists():
            raise
        message = str(exc)
        if ("ProcessSingleton" in message or "profile is already in use" in message) and _cleanup_stale_singleton_artifacts(session_dir):
            trace("retrying persistent browser session after removing stale singleton artifacts")
            return launch(str(session_dir))
        trace(
            f"persistent session launch failed without a recoverable singleton error; preserving {session_dir}"
        )
        raise


def prepare_user_data_dir(
    mode: str, user_data_dir: str | None
) -> tuple[str | None, tempfile.TemporaryDirectory[str] | None]:
    if user_data_dir:
        return user_data_dir, None
    if mode not in {"email-link", "credentials"}:
        return None, None
    temp_dir = tempfile.TemporaryDirectory(prefix="copilot-money-cli-")
    trace(f"using temporary persistent profile dir={temp_dir.name}")
    return temp_dir.name, temp_dir


def _gmail_service():
    _reexec_into_integrations_venv_if_needed()
    sys.path.insert(0, str(Path.home() / ".codex" / "integrations"))
    from mailcal.google.gmail import build_service  # type: ignore

    return build_service()


def extract_links(message: dict) -> list[str]:
    def iter_parts(part: dict):
        children = part.get("parts") or []
        if children:
            for child in children:
                yield from iter_parts(child)
        else:
            yield part

    def decode(part: dict) -> str:
        data = (part.get("body") or {}).get("data")
        if not data:
            return ""
        raw = urlsafe_b64decode(data + "==")
        return raw.decode("utf-8", errors="ignore")

    payload = message.get("payload") or {}
    candidates: list[str] = []
    for part in iter_parts(payload):
        mime = str(part.get("mimeType") or "").lower()
        if mime not in ("text/html", "text/plain"):
            continue
        text = decode(part)
        if not text:
            continue
        for url in re.findall(r'''href=["'](https?://[^"']+)["']''', text, flags=re.I):
            url = html.unescape(url).strip().strip('"\'')
            if "copilot" in url or "/__/auth/action" in url:
                candidates.append(url)
        for url in re.findall(r'''https?://[^\s"<>)]{10,}''', text):
            url = html.unescape(url).strip().strip('"\'')
            if "copilot" in url or "/__/auth/action" in url:
                candidates.append(url)

    firebase = [url for url in candidates if "/__/auth/action" in url and "oobCode=" in url]
    if firebase:
        return sorted(set(firebase), key=len, reverse=True)

    app_links = [url for url in candidates if url.startswith("https://app.copilot.money")]
    return sorted(set(app_links), key=len, reverse=True)


def wait_for_magic_link(*, timeout_seconds: int, email: str | None) -> str:
    trace("connecting to Gmail API")
    service = _gmail_service()
    start_ms = int(time.time() * 1000) - max(300_000, int(timeout_seconds) * 1_000)
    deadline = time.time() + max(5, int(timeout_seconds))
    last_seen = None
    sender_query = "from:(noreply-copilotmoney@copilot.money OR no-reply@copilot.money OR team@copilot.money)"
    queries = [f"newer_than:1d to:{email} {sender_query}".strip()] if email else []
    broader_query = f"newer_than:1d {sender_query}".strip()
    if broader_query not in queries:
        queries.append(broader_query)
    while time.time() < deadline:
        for query in queries:
            trace(f"polling Gmail for magic link query={query}")
            resp = (
                service.users()
                .messages()
                .list(userId="me", q=query, maxResults=5, includeSpamTrash=False)
                .execute()
            )
            ids = [msg.get("id") for msg in (resp.get("messages") or []) if msg.get("id")]
            for message_id in ids:
                message = (
                    service.users()
                    .messages()
                    .get(userId="me", id=message_id, format="full")
                    .execute()
                )
                internal_ms = int(message.get("internalDate") or 0)
                if internal_ms < start_ms:
                    continue
                links = extract_links(message)
                if links:
                    return links[0]
                last_seen = (message_id, internal_ms)
        time.sleep(3)
    raise SystemExit(f"failed to find a fresh Copilot login email (last_seen={last_seen})")


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
        help="Email address (required for non-interactive login unless it can be inferred from --secrets-file).",
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
    if mode in {"email-link", "credentials", "session"}:
        _reexec_into_integrations_venv_if_needed()
    interactive = mode == "interactive"
    email_link = mode == "email-link"
    credentials_mode = mode == "credentials"
    session_mode = mode == "session"
    headful = bool(args.headful) or interactive
    browser_channel = preferred_browser_channel()
    if should_force_headful_for_login(mode, browser_channel=browser_channel):
        headful = True
    secrets_file = Path(args.secrets_file).expanduser()
    email = infer_email(args.email, secrets_file)
    user_data_dir, temp_profile = prepare_user_data_dir(mode, args.user_data_dir)
    if (credentials_mode or email_link) and not email:
        print("--email is required (or must be inferable from --secrets-file)", file=sys.stderr)
        return 2

    token: str | None = None

    _reexec_under_xvfb_if_needed(mode, headful)
    trace(f"starting mode={mode}")
    with sync_playwright() as p:
        context = launch_browser_context(
            p,
            user_data_dir=user_data_dir,
            headful=headful,
        )
        page = context.new_page()

        def on_request(req) -> None:
            nonlocal token
            if token is not None:
                return
            if not req.url.startswith("https://app.copilot.money/api/graphql"):
                return
            auth = req.headers.get("authorization")
            if not auth:
                return
            parts = auth.split(" ", 1)
            if len(parts) == 2 and parts[0].lower() == "bearer":
                token = parts[1].strip()

        page.on("request", on_request)

        def click_continue_with_email() -> None:
            trace("looking for continue-with-email button")
            locators = [
                page.get_by_role("button", name="Continue with email"),
                page.locator('button:has-text("Continue with email")'),
                page.locator('text=Continue with email'),
            ]
            for _ in range(40):
                for loc in locators:
                    try:
                        if loc.count() > 0:
                            trace("clicked continue-with-email button")
                            loc.first.click(force=True)
                            return
                    except Exception:
                        pass
                page.wait_for_timeout(250)

        def fill_email_address(addr: str) -> None:
            trace(f"filling email fields for {addr}")
            selectors = [
                'input[name="email"]',
                'input[name="confirmEmail"]',
                'input[type="email"]',
                'input[autocomplete="email"]',
            ]
            for _ in range(40):
                for selector in selectors:
                    loc = page.locator(selector)
                    try:
                        count = loc.count()
                    except Exception:
                        count = 0
                    if count == 0:
                        continue
                    trace(f"found {count} field(s) for {selector}")
                    for index in range(count):
                        try:
                            field = loc.nth(index)
                            if not field.is_visible():
                                continue
                            field.click(timeout=1000)
                            field.fill(addr, timeout=1000)
                            value = field.input_value(timeout=1000)
                            if value == addr:
                                trace(f"filled {selector} #{index + 1}")
                                return
                        except Exception:
                            continue
                page.wait_for_timeout(250)
            raise SystemExit("could not find email input")

        def click_continue() -> None:
            trace("looking for continue button")
            for name in ["Continue", "Sign in", "Log in", "Send link", "Next"]:
                try:
                    btn = page.get_by_role("button", name=name, exact=False)
                    if btn.count() > 0 and btn.first.is_enabled():
                        trace(f"clicked continue button {name}")
                        btn.first.click()
                        return
                except Exception:
                    pass
            try:
                trace("clicked fallback first button")
                page.locator("button").first.click(force=True)
                return
            except Exception:
                raise SystemExit("could not click Continue")

        def request_email_link(addr: str) -> None:
            try:
                click_continue_with_email()
            except BaseException:
                pass
            page.wait_for_timeout(250)
            fill_email_address(addr)
            click_continue()

        def click_sign_in_with_password() -> None:
            trace("looking for sign-in-with-password button")
            locators = [
                page.get_by_role("button", name="Sign in with password instead"),
                page.locator('button:has-text("Sign in with password instead")'),
                page.locator('text=Sign in with password instead'),
            ]
            for _ in range(40):
                for loc in locators:
                    try:
                        if loc.count() > 0:
                            trace("clicked sign-in-with-password button")
                            loc.first.click(force=True)
                            return
                    except Exception:
                        pass
                page.wait_for_timeout(250)
            raise SystemExit("could not find Sign in with password instead")

        def fill_password(password: str) -> None:
            trace("filling password field")
            locators = [
                page.locator('input[type="password"]'),
                page.locator('input[name="password"]'),
                page.locator('input[autocomplete="current-password"]'),
            ]
            for _ in range(40):
                for loc in locators:
                    try:
                        if loc.count() == 0:
                            continue
                        field = loc.first
                        if not field.is_visible():
                            continue
                        field.click(timeout=1000)
                        field.fill(password, timeout=1000)
                        return
                    except Exception:
                        continue
                page.wait_for_timeout(250)
            raise SystemExit("could not find password input")

        def submit_password_login(addr: str, password: str) -> None:
            try:
                click_continue_with_email()
            except BaseException:
                pass
            page.wait_for_timeout(250)
            fill_email_address(addr)
            click_continue()
            click_sign_in_with_password()
            fill_password(password)
            click_continue()

        def maybe_storage_token() -> str | None:
            try:
                stores = page.evaluate(
                    "() => ({ local: Object.fromEntries(Object.entries(localStorage)), session: Object.fromEntries(Object.entries(sessionStorage)) })"
                )
            except Exception:
                return None

            def scan(obj: object) -> str | None:
                if not isinstance(obj, dict):
                    return None
                for value in obj.values():
                    if not isinstance(value, str):
                        continue
                    raw = value.strip()
                    if raw.lower().startswith("bearer "):
                        raw = raw.split(" ", 1)[1].strip()
                    if raw.startswith("eyJ") and raw.count(".") >= 2 and len(raw) > 200:
                        return raw
                    if "eyJ" in raw and raw.count(".") >= 2 and len(raw) > 200:
                        match = re.search(r'''(eyJ[^\s"']{200,})''', raw)
                        if match:
                            return match.group(1)
                return None

            return scan(stores.get("local")) or scan(stores.get("session"))

        def wait_for_token(timeout_seconds: int) -> str | None:
            deadline = time.time() + max(1, int(timeout_seconds))
            while time.time() < deadline:
                if token and token_is_fresh(token):
                    return token
                storage_token = maybe_storage_token()
                if storage_token and token_is_fresh(storage_token):
                    return storage_token
                page.wait_for_timeout(250)
            return token if token and token_is_fresh(token) else None

        url = "https://app.copilot.money/"
        if email_link or credentials_mode:
            url = "https://app.copilot.money/login"
        trace(f"navigating to {url}")
        page.goto(url, wait_until="domcontentloaded", timeout=60_000)
        if interactive:
            print(
                "Waiting for you to log in in the opened browser window...",
                file=sys.stderr,
            )
        elif credentials_mode:
            password = infer_password(secrets_file)
            if not password:
                print("--mode credentials requires password=... in --secrets-file", file=sys.stderr)
                return 2
            trace("submitting credentials login")
            submit_password_login(email, password)
        elif email_link:
            trace("requesting email link")
            request_email_link(email)
            try:
                trace("waiting for magic link email")
                link = wait_for_magic_link(timeout_seconds=args.timeout_seconds, email=email)
            except Exception:
                link = None
            if not link:
                link = getpass.getpass(
                    "Paste Copilot sign-in link URL from your email (input hidden): "
                ).strip()
                if not link.startswith("http"):
                    print("invalid link", file=sys.stderr)
                    browser.close()
                    return 2
            trace("opening magic link")
            page.goto(link, wait_until="domcontentloaded", timeout=60_000)
            email_confirmation_fields = page.locator('input[type="email"]')
            try:
                email_confirmation_count = email_confirmation_fields.count()
            except Exception:
                email_confirmation_count = 0
            if email_confirmation_count:
                trace("filling email on auth/link confirmation")
                fill_email_address(email)
                trace("confirming email link")
                click_continue()
                page.wait_for_timeout(1000)
            else:
                trace("auth/link confirmation email field not present; continuing")
            try:
                trace("opening transactions after magic link")
                page.goto(
                    "https://app.copilot.money/transactions",
                    wait_until="domcontentloaded",
                    timeout=60_000,
                )
            except Exception:
                pass

        initial_wait_seconds = 5 if session_mode else args.timeout_seconds
        trace(f"waiting for token for up to {initial_wait_seconds}s")
        captured = wait_for_token(initial_wait_seconds)

        if session_mode and not captured:
            try:
                trace("session mode did not capture token on landing page; opening transactions route")
                page.goto(
                    "https://app.copilot.money/transactions",
                    wait_until="domcontentloaded",
                    timeout=60_000,
                )
            except Exception:
                trace("failed to open transactions route during session refresh")

        trace(f"waiting for token for up to {args.timeout_seconds}s after magic link")
        captured = wait_for_token(args.timeout_seconds)

        page.context.close()

    token = captured or token
    if not token:
        if temp_profile is not None:
            temp_profile.cleanup()
        if session_mode and user_data_dir:
            print(
                "failed to capture token using persisted session; run `copilot auth login --persist-session` explicitly",
                file=sys.stderr,
            )
        else:
            print("failed to capture token", file=sys.stderr)
        return 1

    if temp_profile is not None:
        temp_profile.cleanup()
    sys.stdout.write(token)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
