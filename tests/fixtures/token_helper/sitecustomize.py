"""Network-free Playwright and Gmail doubles for the Rust/helper transport test."""

from __future__ import annotations

import base64
import os
import sys
import time
import types


# Build a synthetic unsigned JWT so the fixture contains no secret-like literal.
TOKEN = ".".join(
    [
        base64.urlsafe_b64encode(b'{"alg":"none"}').decode("ascii").rstrip("="),
        base64.urlsafe_b64encode(b'{"exp":4102444800}').decode("ascii").rstrip("="),
        "fixture",
    ]
)
MAGIC_LINK = "https://app.copilot.money/__/auth/action?mode=signIn&oobCode=fixture"


def _record(event: str) -> None:
    path = os.environ.get("COPILOT_TEST_TRACE")
    if path:
        with open(path, "a", encoding="utf-8") as trace_file:
            trace_file.write(event + "\n")


class _Request:
    url = "https://app.copilot.money/api/graphql"
    headers = {"authorization": f"Bearer {TOKEN}"}


class _Element:
    def __init__(
        self,
        *,
        visible: bool = True,
        enabled: bool = True,
        on_fill=None,
        on_click=None,
    ) -> None:
        self.visible = visible
        self.enabled = enabled
        self.on_fill = on_fill
        self.on_click = on_click
        self.value = ""

    def click(self, *args, **kwargs) -> None:
        if self.on_click is not None:
            self.on_click()
        return None

    def fill(self, value: str, *args, **kwargs) -> None:
        self.value = value
        if self.on_fill is not None:
            self.on_fill(value)
        if value == "fixture@example.test":
            _record("filled-email:fixture@example.test")
        else:
            _record("filled-email:unexpected")

    def is_visible(self) -> bool:
        return self.visible

    def is_enabled(self) -> bool:
        return self.enabled

    def input_value(self, *args, **kwargs) -> str:
        return self.value


class _Locator:
    def __init__(self, elements: list[_Element]) -> None:
        self.elements = elements

    def count(self) -> int:
        return len(self.elements)

    @property
    def first(self) -> _Element:
        return self.elements[0]

    def nth(self, index: int) -> _Element:
        return self.elements[index]


class _Page:
    def __init__(self, context: "_Context") -> None:
        self.context = context
        self.handlers: dict[str, object] = {}
        self.email_button = _Element()
        self.email_field = _Element()
        self.password_button = _Element()
        self.password_filled = False
        self.password_field = _Element(on_fill=self._record_password)
        self.continue_button = _Element(on_click=self._submit_password_login)

    def _record_password(self, value: str) -> None:
        self.password_filled = value == "fixture-only"
        _record("filled-password:fixture" if self.password_filled else "filled-password:unexpected")

    def _submit_password_login(self) -> None:
        if not self.password_filled:
            return
        handler = self.handlers.get("request")
        if handler is not None:
            _record("browser:fixture-request-token")
            handler(_Request())

    def on(self, name: str, handler) -> None:
        self.handlers[name] = handler

    def goto(self, url: str, **kwargs) -> None:
        if url == "https://app.copilot.money/login":
            _record("browser-navigation:login")
            return
        if url.startswith(MAGIC_LINK):
            _record("browser-navigation:fixture-magic-link")
            handler = self.handlers.get("request")
            if handler is not None:
                handler(_Request())
            return
        if url == "https://app.copilot.money/transactions":
            _record("browser-navigation:transactions")
            return
        raise AssertionError("external browser navigation is blocked in this fixture")

    def wait_for_timeout(self, milliseconds: int) -> None:
        return None

    def evaluate(self, script: str) -> dict[str, dict]:
        return {"local": {}, "session": {}}

    def get_by_role(self, role: str, name: str, exact=None) -> _Locator:
        if role == "button" and name == "Continue with email":
            return _Locator([self.email_button])
        if role == "button" and name == "Sign in with password instead":
            return _Locator([self.password_button])
        if role == "button" and name in {"Continue", "Send link", "Next"}:
            return _Locator([self.continue_button])
        return _Locator([])

    def locator(self, selector: str) -> _Locator:
        if selector in {'input[name="email"]', 'input[type="email"]', 'input[autocomplete="email"]'}:
            return _Locator([self.email_field])
        if selector in {'input[type="password"]', 'input[name="password"]', 'input[autocomplete="current-password"]'}:
            return _Locator([self.password_field])
        if selector == 'input[name="confirmEmail"]':
            return _Locator([])
        if selector == "button":
            return _Locator([self.continue_button])
        return _Locator([])


class _Context:
    def __init__(self) -> None:
        self.page = _Page(self)

    def new_page(self) -> _Page:
        return self.page

    def close(self) -> None:
        _record("browser-context:closed")


class _Chromium:
    def launch(self, **kwargs):
        raise AssertionError("the fixture must not launch a real browser")

    def launch_persistent_context(self, *args, **kwargs) -> _Context:
        _record("browser:stubbed-no-network")
        return _Context()


class _Playwright:
    def __init__(self) -> None:
        self.chromium = _Chromium()


class _PlaywrightContextManager:
    def __enter__(self) -> _Playwright:
        return _Playwright()

    def __exit__(self, exc_type, exc_value, traceback) -> bool:
        return False


def sync_playwright() -> _PlaywrightContextManager:
    return _PlaywrightContextManager()


class _Execute:
    def __init__(self, result: dict) -> None:
        self.result = result

    def execute(self) -> dict:
        return self.result


class _Messages:
    def list(self, **kwargs) -> _Execute:
        query = str(kwargs.get("q", ""))
        if "to:fixture@example.test" not in query:
            raise AssertionError("fixture Gmail query did not use the descriptor email")
        _record("gmail:stubbed-query-to-fixture-email")
        return _Execute({"messages": [{"id": "fixture-message"}]})

    def get(self, **kwargs) -> _Execute:
        html = f'<a href="{MAGIC_LINK}">fixture magic link</a>'
        encoded = base64.urlsafe_b64encode(html.encode("utf-8")).decode("ascii").rstrip("=")
        return _Execute(
            {
                "internalDate": str(int(time.time() * 1000)),
                "payload": {
                    "mimeType": "text/html",
                    "body": {"data": ""},
                    "parts": [{
                        "mimeType": "text/html",
                        "body": {"data": encoded},
                    }],
                },
            }
        )


class _Users:
    def messages(self) -> _Messages:
        return _Messages()


class _GmailService:
    def users(self) -> _Users:
        return _Users()


def _build_service() -> _GmailService:
    _record("gmail:stubbed-no-network")
    return _GmailService()


playwright_module = types.ModuleType("playwright")
playwright_module.__path__ = []
sync_api_module = types.ModuleType("playwright.sync_api")
sync_api_module.sync_playwright = sync_playwright
playwright_module.sync_api = sync_api_module
sys.modules["playwright"] = playwright_module
sys.modules["playwright.sync_api"] = sync_api_module

mailcal_module = types.ModuleType("mailcal")
mailcal_module.__path__ = []
google_module = types.ModuleType("mailcal.google")
google_module.__path__ = []
gmail_module = types.ModuleType("mailcal.google.gmail")
gmail_module.build_service = _build_service
mailcal_module.google = google_module
google_module.gmail = gmail_module
sys.modules["mailcal"] = mailcal_module
sys.modules["mailcal.google"] = google_module
sys.modules["mailcal.google.gmail"] = gmail_module
