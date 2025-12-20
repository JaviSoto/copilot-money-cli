#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import re
import time
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Any

from playwright.sync_api import sync_playwright


@dataclass(frozen=True)
class Credentials:
    email: str
    password: str


def _now_slug() -> str:
    return datetime.now().strftime("%Y%m%d-%H%M%S")


def load_credentials(path: Path) -> Credentials:
    email: str | None = None
    password: str | None = None
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if "=" not in line:
            continue
        key, value = line.split("=", 1)
        key = key.strip().lower()
        value = value.strip()
        if key == "email":
            email = value
        elif key == "password":
            password = value
    if not email or not password:
        raise SystemExit(f"Missing email/password in {path}")
    return Credentials(email=email, password=password)


def safe_filename(s: str) -> str:
    s = s.strip()
    if not s:
        return "unknown"
    s = re.sub(r"[^a-zA-Z0-9._-]+", "_", s)
    return s[:120] if len(s) > 120 else s


def _normalize_query(query: str) -> str:
    return " ".join(query.strip().split())


def _stable_id(operation_name: str, query: str) -> str:
    digest = hashlib.sha256(_normalize_query(query).encode("utf-8")).hexdigest()[:16]
    return f"{operation_name}:{digest}"


def _operation_kind(query: str) -> str:
    m = re.match(r"^\\s*(query|mutation|subscription)\\b", query)
    return m.group(1) if m else "unknown"


def _try_request_json(req) -> dict[str, Any] | None:
    attr = getattr(req, "post_data_json", None)
    try:
        if callable(attr):
            payload = attr()
        else:
            payload = attr
    except Exception:
        payload = None

    if isinstance(payload, dict):
        return payload

    # Fallback: parse raw body as JSON if available.
    try:
        raw = req.post_data()
    except Exception:
        raw = None
    if not isinstance(raw, str) or not raw.strip():
        return None
    try:
        parsed = json.loads(raw)
    except Exception:
        return None
    return parsed if isinstance(parsed, dict) else None


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Capture Copilot Money GraphQL operation documents (no variable values)."
    )
    parser.add_argument(
        "--secrets-file",
        default=str(Path("~/.codex/secrets/copilot_money").expanduser()),
        help="Path to secrets file containing email=... and password=...",
    )
    parser.add_argument(
        "--out-dir",
        default=str(Path("artifacts/graphql-ops").resolve()),
        help="Directory to write captures to",
    )
    parser.add_argument(
        "--headful",
        action="store_true",
        help="Run browser headful (useful for debugging)",
    )
    parser.add_argument(
        "--settle-seconds",
        type=int,
        default=6,
        help="Seconds to wait on the dashboard to let requests fire",
    )
    parser.add_argument(
        "--debug",
        action="store_true",
        help="Print capture diagnostics (no secrets)",
    )

    args = parser.parse_args()

    secrets_path = Path(args.secrets_file).expanduser()
    creds = load_credentials(secrets_path)

    out_root = Path(args.out_dir)
    run_dir = out_root / _now_slug()
    run_dir.mkdir(parents=True, exist_ok=True)

    ops: dict[str, dict[str, Any]] = {}
    dbg = {
        "graphql_seen": 0,
        "graphql_parsed": 0,
        "graphql_skipped_no_query": 0,
        "graphql_skipped_introspection": 0,
    }

    def on_request(req) -> None:
        if req.url != "https://app.copilot.money/api/graphql":
            return
        if req.method.upper() != "POST":
            return
        dbg["graphql_seen"] += 1

        payload = _try_request_json(req)
        if not isinstance(payload, dict):
            return

        dbg["graphql_parsed"] += 1
        query = payload.get("query")
        if not isinstance(query, str):
            dbg["graphql_skipped_no_query"] += 1
            return
        # Skip schema introspection, but do not treat `__typename` as introspection.
        if re.search(r"__schema\\b", query) or re.search(r"__type\\b", query):
            dbg["graphql_skipped_introspection"] += 1
            return

        operation_name = payload.get("operationName")
        if not isinstance(operation_name, str) or not operation_name.strip():
            operation_name = "anonymous"

        variables = payload.get("variables")
        variable_keys: list[str] = []
        if isinstance(variables, dict):
            variable_keys = sorted([str(k) for k in variables.keys()])

        kind = _operation_kind(query)
        key = _stable_id(operation_name, query)
        ops[key] = {
            "operationName": operation_name,
            "stableId": key,
            "kind": kind,
            "url": req.url,
            "method": req.method,
            "variableKeys": variable_keys,
            "query": query,
        }

    with sync_playwright() as p:
        browser = p.chromium.launch(headless=not args.headful)
        context = browser.new_context(viewport={"width": 1280, "height": 720})
        page = context.new_page()
        page.on("request", on_request)

        page.goto("https://app.copilot.money/", wait_until="domcontentloaded", timeout=60_000)
        page.get_by_role("button", name="Continue with email").click()
        page.get_by_placeholder("Email address").fill(creds.email)
        page.get_by_role("button", name="Continue", exact=True).click()

        page.get_by_role("button", name="Sign in with password instead").click()
        page.locator('input[type="password"]').first.fill(creds.password)
        for name in ["Sign in", "Continue", "Log in"]:
            btn = page.get_by_role("button", name=name)
            if btn.count() > 0:
                btn.first.click()
                break

        page.wait_for_load_state("domcontentloaded", timeout=60_000)
        time.sleep(max(0, int(args.settle_seconds)))

        page.screenshot(path=str(run_dir / "dashboard.png"), full_page=True)
        page.goto(
            "https://app.copilot.money/transactions",
            wait_until="domcontentloaded",
            timeout=60_000,
        )
        time.sleep(max(0, int(args.settle_seconds)))
        page.screenshot(path=str(run_dir / "transactions.png"), full_page=True)
        browser.close()

    # Write unique operations grouped by (operationName, query).
    manifest_path = run_dir / "operations.json"
    manifest_path.write_text(
        json.dumps(
            {
                "capturedAt": datetime.now().isoformat(),
                "count": len(ops),
                "debug": dbg,
                "operations": list(ops.values()),
            },
            indent=2,
            sort_keys=True,
        ),
        encoding="utf-8",
    )

    gql_dir = run_dir / "graphql"
    gql_dir.mkdir(parents=True, exist_ok=True)
    for item in ops.values():
        stable_id = item.get("stableId") or _stable_id(item["operationName"], item["query"])
        digest = str(stable_id).split(":")[-1]
        op = safe_filename(item["operationName"])
        query = item["query"]
        (gql_dir / f"{op}--{digest}.graphql").write_text(query.strip() + "\n", encoding="utf-8")

    if args.debug:
        print(json.dumps(dbg, indent=2, sort_keys=True))

    print(f"Wrote {len(ops)} operations to {run_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
