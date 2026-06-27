from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import get_token


class GetTokenTests(unittest.TestCase):
    def test_infer_email_prefers_explicit_value(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            secrets = Path(tmp) / "copilot_money"
            secrets.write_text("email=fixture@example.com\n", encoding="utf-8")
            self.assertEqual(
                get_token.infer_email("real@example.com", secrets),
                "real@example.com",
            )

    def test_infer_email_reads_secrets_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            secrets = Path(tmp) / "copilot_money"
            secrets.write_text("email=pilotapp@javisoto.es\npassword=redacted\n", encoding="utf-8")
            self.assertEqual(
                get_token.infer_email(None, secrets),
                "pilotapp@javisoto.es",
            )

    def test_infer_password_reads_secrets_file_for_credentials_mode(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            secrets = Path(tmp) / "copilot_money"
            secrets.write_text("email=pilotapp@javisoto.es\npassword=secret-value\n", encoding="utf-8")
            self.assertEqual(get_token.infer_password(secrets), "secret-value")

    def test_should_force_headful_for_login_with_chrome_on_darwin(self) -> None:
        with mock.patch.object(get_token.platform, "system", return_value="Darwin"):
            self.assertTrue(
                get_token.should_force_headful_for_login("credentials", browser_channel="chrome")
            )
            self.assertTrue(
                get_token.should_force_headful_for_login("email-link", browser_channel="chrome")
            )
            self.assertFalse(
                get_token.should_force_headful_for_login("session", browser_channel="chrome")
            )

    def test_should_not_force_headful_without_display_on_linux(self) -> None:
        with mock.patch.object(get_token.platform, "system", return_value="Linux"):
            with mock.patch.dict(get_token.os.environ, {}, clear=True):
                self.assertFalse(
                    get_token.should_force_headful_for_login("credentials", browser_channel="chrome")
                )

    def test_extract_links_prefers_firebase_magic_link(self) -> None:
        html = (
            '<a href="https://app.copilot.money/login">Open Copilot</a>'
            '<a href="https://example.com/__/auth/action?mode=signIn&oobCode=abc&apiKey=123">'
            'Magic link</a>'
        )
        payload = {
            "payload": {
                "mimeType": "text/html",
                "body": {"data": ""},
                "parts": [
                    {
                        "mimeType": "text/html",
                        "body": {
                            "data": __import__("base64").urlsafe_b64encode(html.encode("utf-8")).decode("utf-8")
                        },
                    }
                ],
            }
        }
        links = get_token.extract_links(payload)
        self.assertEqual(
            links[0],
            "https://example.com/__/auth/action?mode=signIn&oobCode=abc&apiKey=123",
        )


    def test_prepare_user_data_dir_creates_temp_profile_for_credentials_mode(self) -> None:
        profile_dir, temp_profile = get_token.prepare_user_data_dir("credentials", None)

        try:
            self.assertIsNotNone(profile_dir)
            self.assertIsNotNone(temp_profile)
            self.assertTrue(Path(profile_dir).exists())
        finally:
            if temp_profile is not None:
                temp_profile.cleanup()

    def test_prepare_user_data_dir_keeps_explicit_profile(self) -> None:
        profile_dir, temp_profile = get_token.prepare_user_data_dir("credentials", "/tmp/copilot-profile")

        self.assertEqual(profile_dir, "/tmp/copilot-profile")
        self.assertIsNone(temp_profile)

    def test_launch_browser_context_clears_stale_singleton_locks_before_quarantine(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            profile = Path(tmp) / "playwright-session"
            default_dir = profile / "Default"
            default_dir.mkdir(parents=True)
            (profile / "SingletonCookie").symlink_to("12345")
            (profile / "SingletonLock").symlink_to("miniserver-123")
            (profile / "SingletonSocket").symlink_to("/tmp/fake-socket")
            (default_dir / "LOCK").write_text("", encoding="utf-8")

            class _Chromium:
                def __init__(self):
                    self.calls = 0

                def launch(self, **kwargs):
                    raise AssertionError("persistent profile path should be used")

                def launch_persistent_context(self, dir_value, **kwargs):
                    self.calls += 1
                    if self.calls == 1:
                        raise Exception(
                            "BrowserType.launch_persistent_context: Failed to create a ProcessSingleton for your profile directory. This usually means that the profile is already in use by another instance of Chromium."
                        )
                    return {"dir": dir_value}

            class _Playwright:
                def __init__(self):
                    self.chromium = _Chromium()

            playwright = _Playwright()
            context = get_token.launch_browser_context(
                playwright,
                user_data_dir=str(profile),
                headful=False,
            )

            self.assertEqual(context, {"dir": str(profile)})
            self.assertEqual(playwright.chromium.calls, 2)
            self.assertFalse((profile / "SingletonCookie").exists())
            self.assertFalse((profile / "SingletonLock").exists())
            self.assertFalse((profile / "SingletonSocket").exists())
            self.assertFalse((default_dir / "LOCK").exists())
            self.assertEqual(list(profile.parent.glob(f"{profile.name}.broken-*")), [])

    def test_launch_browser_context_does_not_quarantine_profile_on_non_singleton_error(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            profile = Path(tmp) / "playwright-session"
            profile.mkdir(parents=True)
            marker = profile / "Cookies"
            marker.write_text("cookie-state", encoding="utf-8")

            class _Chromium:
                def launch(self, **kwargs):
                    raise AssertionError("persistent profile path should be used")

                def launch_persistent_context(self, dir_value, **kwargs):
                    raise Exception("BrowserType.launch_persistent_context: sandbox init failed")

            class _Playwright:
                def __init__(self):
                    self.chromium = _Chromium()

            with self.assertRaisesRegex(Exception, "sandbox init failed"):
                get_token.launch_browser_context(
                    _Playwright(),
                    user_data_dir=str(profile),
                    headful=False,
                )

            self.assertTrue(profile.exists())
            self.assertEqual(marker.read_text(encoding="utf-8"), "cookie-state")
            self.assertEqual(list(profile.parent.glob(f"{profile.name}.broken-*")), [])


    def test_wait_for_magic_link_accepts_recent_message_before_poll_start(self) -> None:
        html = '<a href="https://auth.copilot.money/__/auth/action?mode=signIn&oobCode=abc">Magic link</a>'
        message = {
            "id": "msg-1",
            "internalDate": str((1_000_000 - 10) * 1000),
            "payload": {
                "mimeType": "text/html",
                "body": {"data": ""},
                "parts": [
                    {
                        "mimeType": "text/html",
                        "body": {
                            "data": __import__("base64").urlsafe_b64encode(html.encode("utf-8")).decode("utf-8")
                        },
                    }
                ],
            },
        }

        class _Execute:
            def __init__(self, payload):
                self.payload = payload

            def execute(self):
                return self.payload

        class _Messages:
            def list(self, **kwargs):
                return _Execute({"messages": [{"id": "msg-1"}]})

            def get(self, **kwargs):
                return _Execute(message)

        class _Users:
            def messages(self):
                return _Messages()

        class _Service:
            def users(self):
                return _Users()

        with mock.patch.object(get_token, '_gmail_service', return_value=_Service()):
            with mock.patch.object(get_token.time, 'time', side_effect=[1_000_000, 1_000_000, 1_000_000]):
                with mock.patch.object(get_token.time, 'sleep', return_value=None):
                    link = get_token.wait_for_magic_link(timeout_seconds=20, email='pilotapp@javisoto.es')

        self.assertEqual(
            link,
            'https://auth.copilot.money/__/auth/action?mode=signIn&oobCode=abc',
        )

    def test_wait_for_magic_link_falls_back_when_alias_to_filter_misses_forwarded_mail(self) -> None:
        html = '<a href="https://auth.copilot.money/__/auth/action?mode=signIn&oobCode=abc">Magic link</a>'
        message = {
            "id": "msg-1",
            "internalDate": str((1_000_000 - 10) * 1000),
            "payload": {
                "mimeType": "text/html",
                "body": {"data": ""},
                "parts": [
                    {
                        "mimeType": "text/html",
                        "body": {
                            "data": __import__("base64").urlsafe_b64encode(html.encode("utf-8")).decode("utf-8")
                        },
                    }
                ],
            },
        }
        queries: list[str] = []

        class _Execute:
            def __init__(self, payload):
                self.payload = payload

            def execute(self):
                return self.payload

        class _Messages:
            def list(self, **kwargs):
                query = kwargs["q"]
                queries.append(query)
                if "to:pilotapp@javisoto.es" in query:
                    return _Execute({})
                return _Execute({"messages": [{"id": "msg-1"}]})

            def get(self, **kwargs):
                return _Execute(message)

        class _Users:
            def messages(self):
                return _Messages()

        class _Service:
            def users(self):
                return _Users()

        with mock.patch.object(get_token, "_gmail_service", return_value=_Service()):
            with mock.patch.object(get_token.time, "time", side_effect=[1_000_000, 1_000_000, 1_000_000, 1_000_021]):
                with mock.patch.object(get_token.time, "sleep", return_value=None):
                    link = get_token.wait_for_magic_link(timeout_seconds=20, email="pilotapp@javisoto.es")

        self.assertEqual(
            link,
            "https://auth.copilot.money/__/auth/action?mode=signIn&oobCode=abc",
        )
        self.assertIn("to:pilotapp@javisoto.es", queries[0])
        self.assertNotIn("to:pilotapp@javisoto.es", queries[1])

    def test_session_mode_fails_without_requesting_magic_link_when_session_capture_fails(self) -> None:
        class _Element:
            def __init__(self, *, visible=True, enabled=True):
                self._visible = visible
                self._enabled = enabled
                self.clicks = 0
                self.fills: list[str] = []

            def click(self, *args, **kwargs):
                self.clicks += 1

            def fill(self, value, *args, **kwargs):
                self.fills.append(value)

            def is_visible(self):
                return self._visible

            def is_enabled(self):
                return self._enabled

        class _Locator:
            def __init__(self, elements):
                self.elements = elements

            def count(self):
                return len(self.elements)

            @property
            def first(self):
                return self.elements[0]

            def nth(self, index):
                return self.elements[index]

        class _Page:
            def __init__(self):
                self.url = None
                self.request_handler = None
                self.goto_calls: list[str] = []
                self.context = mock.Mock()
                self.visible_email = _Element(visible=True)
                self.hidden_confirm = _Element(visible=False)
                self.continue_button = _Element(visible=True, enabled=True)

            def on(self, name, handler):
                self.request_handler = handler

            def goto(self, url, **kwargs):
                self.url = url
                self.goto_calls.append(url)

            def wait_for_timeout(self, ms):
                return None

            def evaluate(self, script):
                return {"local": {}, "session": {}}

            def get_by_role(self, role, name=None, exact=None):
                if role != "button":
                    return _Locator([])
                if name in {"Continue", "Send link", "Next"}:
                    return _Locator([self.continue_button])
                return _Locator([])

            def locator(self, selector):
                mapping = {
                    'input[name="email"]': [self.visible_email],
                    'input[name="confirmEmail"]': [self.hidden_confirm],
                    'input[type="email"]': [],
                    'input[autocomplete="email"]': [],
                    'button': [self.continue_button],
                }
                return _Locator(mapping.get(selector, []))

        class _Context:
            def __init__(self, page):
                self._page = page

            def new_page(self):
                return self._page

            def close(self):
                pass

        class _Chromium:
            def __init__(self, context):
                self._context = context

            def launch(self, **kwargs):
                raise AssertionError("session mode should not use ephemeral browser")

            def launch_persistent_context(self, *args, **kwargs):
                return self._context

        class _Playwright:
            def __init__(self, context):
                self.chromium = _Chromium(context)

        class _PlaywrightCM:
            def __init__(self, playwright):
                self.playwright = playwright

            def __enter__(self):
                return self.playwright

            def __exit__(self, exc_type, exc, tb):
                return False

        page = _Page()
        context = _Context(page)
        playwright = _Playwright(context)

        with tempfile.TemporaryDirectory() as tmp:
            secrets = Path(tmp) / "copilot_money"
            secrets.write_text("email=pilotapp@javisoto.es\n", encoding="utf-8")
            argv = [
                "get_token.py",
                "--mode",
                "session",
                "--timeout-seconds",
                "1",
                "--secrets-file",
                str(secrets),
                "--user-data-dir",
                str(Path(tmp) / "profile"),
            ]
            with mock.patch.object(get_token, "sync_playwright", return_value=_PlaywrightCM(playwright)):
                with mock.patch.object(get_token, "_reexec_into_integrations_venv_if_needed", return_value=None):
                    with mock.patch.object(get_token, "_reexec_under_xvfb_if_needed", return_value=None):
                        with mock.patch.object(get_token, "wait_for_magic_link", side_effect=AssertionError("session mode must not poll Gmail for a magic link")):
                            with mock.patch.object(get_token.sys, "argv", argv):
                                with mock.patch.object(get_token, "token_is_fresh", return_value=False):
                                    rc = get_token.main()

        self.assertEqual(rc, 1)
        self.assertEqual(page.visible_email.fills, [])
        self.assertEqual(page.hidden_confirm.fills, [])
        self.assertEqual(
            page.goto_calls,
            [
                "https://app.copilot.money/",
                "https://app.copilot.money/transactions",
            ],
        )


if __name__ == "__main__":
    unittest.main()
