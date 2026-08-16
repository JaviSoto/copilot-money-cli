#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import re
import sys
import urllib.parse
import urllib.request
import urllib.error
from pathlib import Path

# Public Firebase web API key used by app.copilot.money.
# This is a client-side identifier, not a secret.
FIREBASE_API_KEY = "AIzaSyAMgjkeOSkHj4J4rlswOkD16N3WQOoNPpk"
TOKEN_ENDPOINT = (
    "https://securetoken.googleapis.com/v1/token?key=" + FIREBASE_API_KEY
)

REFRESH_TOKEN_RE = re.compile(rb"AMf-[A-Za-z0-9_-]{100,}")

SAFARI_PATHS = [
    Path("~/Library/Containers/com.apple.Safari/Data/Library/WebKit/WebsiteData/Default").expanduser(),
    Path("~/Library/Safari/Databases").expanduser(),
]

# Also support Chromium-family browsers so the helper remains useful if the
# Copilot session is moved out of Safari later.
CHROMIUM_PATHS = [
    ("Chrome", Path("~/Library/Application Support/Google/Chrome/Default/IndexedDB/https_app.copilot.money_0.indexeddb.leveldb").expanduser()),
    ("Chrome", Path("~/Library/Application Support/Google/Chrome/Default/Local Storage/leveldb").expanduser()),
    ("Chrome Profile 1", Path("~/Library/Application Support/Google/Chrome/Profile 1/IndexedDB/https_app.copilot.money_0.indexeddb.leveldb").expanduser()),
    ("Chrome Profile 1", Path("~/Library/Application Support/Google/Chrome/Profile 1/Local Storage/leveldb").expanduser()),
    ("Arc", Path("~/Library/Application Support/Arc/User Data/Default/IndexedDB/https_app.copilot.money_0.indexeddb.leveldb").expanduser()),
    ("Arc", Path("~/Library/Application Support/Arc/User Data/Default/Local Storage/leveldb").expanduser()),
]


def find_tokens_in_file(path: Path) -> list[str]:
    try:
        size = path.stat().st_size
        if size <= 0 or size > 10_000_000:
            return []
        data = path.read_bytes()
    except (OSError, PermissionError):
        return []

    return [m.decode("ascii") for m in REFRESH_TOKEN_RE.findall(data)]


def search_dir(root: Path, max_depth: int = 6) -> list[str]:
    if not root.exists():
        return []

    tokens: list[str] = []
    root_depth = len(root.parts)

    try:
        for dirpath, dirnames, filenames in os.walk(root, topdown=True):
            current = Path(dirpath)
            depth = len(current.parts) - root_depth
            if depth >= max_depth:
                dirnames[:] = []

            # Skip obvious large/noisy directories where possible.
            dirnames[:] = [
                d for d in dirnames
                if d not in {"Cache", "Caches", "ServiceWorkers", "NetworkCache"}
            ]

            for name in filenames:
                path = current / name
                tokens.extend(find_tokens_in_file(path))
    except (OSError, PermissionError):
        pass

    return tokens


def extract_refresh_token(browser: str) -> tuple[str, str]:
    candidates: list[tuple[str, str]] = []

    if browser in {"auto", "safari"}:
        for path in SAFARI_PATHS:
            for token in search_dir(path):
                candidates.append(("Safari", token))

    if browser in {"auto", "chromium"}:
        for browser_name, path in CHROMIUM_PATHS:
            if path.is_dir():
                # LevelDB directories are shallow; no need for a deep walk.
                for token in search_dir(path, max_depth=2):
                    candidates.append((browser_name, token))
            elif path.is_file():
                for token in find_tokens_in_file(path):
                    candidates.append((browser_name, token))

    if not candidates:
        safari_hint = (
            "\nSafari note: macOS may block Terminal from reading Safari's "
            "website data. If you are logged into app.copilot.money in Safari "
            "and no token is found, enable Full Disk Access for your terminal "
            "app in System Settings > Privacy & Security > Full Disk Access, "
            "then try again."
        )
        raise RuntimeError(
            "No Copilot Money Firebase refresh token found. "
            "Log into https://app.copilot.money in the selected browser first."
            + safari_hint
        )

    # Prefer the longest token. This mirrors the strategy used by the maintained
    # Copilot Money MCP integration and tends to select the newest Firebase token.
    source, token = max(candidates, key=lambda item: len(item[1]))
    return source, token


def exchange_refresh_token(refresh_token: str) -> dict:
    body = urllib.parse.urlencode(
        {
            "grant_type": "refresh_token",
            "refresh_token": refresh_token,
        }
    ).encode("utf-8")

    request = urllib.request.Request(
        TOKEN_ENDPOINT,
        data=body,
        headers={"Content-Type": "application/x-www-form-urlencoded"},
        method="POST",
    )

    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            payload = json.loads(response.read().decode("utf-8"))
    except urllib.error.HTTPError as exc:
        try:
            detail = exc.read().decode("utf-8", errors="replace")
        except Exception:
            detail = ""
        raise RuntimeError(
            f"Firebase token exchange failed ({exc.code}): {detail}"
        ) from exc
    except urllib.error.URLError as exc:
        raise RuntimeError(f"Firebase token exchange failed: {exc}") from exc

    id_token = payload.get("id_token")
    if not isinstance(id_token, str) or not id_token:
        raise RuntimeError("Firebase response did not contain an id_token")

    return payload


def main() -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Read the existing Copilot Money Firebase session from the browser "
            "and print a fresh API bearer/ID token."
        )
    )

    # Keep the old helper's arguments so copilot-money-cli can call this as a
    # drop-in replacement. Most are intentionally ignored.
    parser.add_argument(
        "--mode",
        choices=["interactive", "email-link", "credentials", "session"],
        default="interactive",
    )
    parser.add_argument("--secrets-file")
    parser.add_argument("--email")
    parser.add_argument("--headful", action="store_true")
    parser.add_argument("--user-data-dir")
    parser.add_argument("--timeout-seconds", type=int, default=180)

    parser.add_argument(
        "--browser",
        choices=["auto", "safari", "chromium"],
        default="auto",
        help="Browser storage to inspect (default: auto).",
    )
    args = parser.parse_args()

    try:
        source, refresh_token = extract_refresh_token(args.browser)
        payload = exchange_refresh_token(refresh_token)
    except Exception as exc:
        print(str(exc), file=sys.stderr)
        return 1

    # IMPORTANT: stdout must contain ONLY the ID token because the Rust CLI
    # captures stdout verbatim and saves it as the bearer token.
    print(f"Using Firebase session from {source}", file=sys.stderr)
    sys.stdout.write(payload["id_token"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
