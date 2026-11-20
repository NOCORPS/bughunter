from __future__ import annotations

import json
import threading
import unittest
from http.server import BaseHTTPRequestHandler, HTTPServer

from bughunter_sdk import (
    AuthenticationError,
    BugHunterClient,
    NotFoundError,
)


class _MockHandler(BaseHTTPRequestHandler):
    def log_message(self, format: str, *args: object) -> None:
        return

    def do_GET(self) -> None:
        if self.headers.get("Authorization") != "Bearer secret-key" and self.path != "/api/v1/health":
            self.send_response(401)
            self.end_headers()
            self.wfile.write(json.dumps({"error": {"code": "AUTHENTICATION_ERROR", "message": "unauthorized"}}).encode())
            return

        if self.path == "/api/v1/health":
            self._json(200, {"status": "ok", "version": "0.1.0", "uptime_seconds": 1, "active_investigations": 0, "total_investigations": 1})
        elif self.path == "/api/v1/investigations/inv-1":
            self._json(200, {
                "id": "inv-1",
                "status": "completed",
                "request": {"repository": "owner/repo"},
                "result": {"summary": "done"},
            })
        elif self.path == "/api/v1/investigations/inv-1/report":
            self._json(200, {
                "investigation_id": "inv-1",
                "title": "Report",
                "executive_summary": "All checks passed",
                "workflow": "investigate",
                "status": "completed",
                "findings": ["root cause found"],
                "recommendations": ["merge"],
            })
        else:
            self._json(404, {"error": {"code": "NOT_FOUND", "message": "not found"}})

    def do_POST(self) -> None:
        if self.headers.get("Authorization") != "Bearer secret-key":
            self._json(401, {"error": {"code": "AUTHENTICATION_ERROR", "message": "unauthorized"}})
            return
        if self.path == "/api/v1/investigations":
            self._json(202, {"id": "inv-1", "status": "running", "request": {"repository": "owner/repo"}})
        elif self.path == "/api/v1/investigations/inv-1/cancel":
            self._json(200, {"id": "inv-1", "status": "cancelled", "request": {"repository": "owner/repo"}})
        else:
            self._json(404, {"error": {"code": "NOT_FOUND", "message": "not found"}})

    def _json(self, code: int, payload: dict) -> None:
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(json.dumps(payload).encode())


class TestPythonSdk(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.server = HTTPServer(("127.0.0.1", 0), _MockHandler)
        cls.port = cls.server.server_address[1]
        cls.thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()

    @classmethod
    def tearDownClass(cls) -> None:
        cls.server.shutdown()
        cls.server.server_close()

    def test_investigation_lifecycle(self) -> None:
        client = BugHunterClient(f"http://127.0.0.1:{self.port}", api_key="secret-key")
        health = client.health()
        self.assertEqual(health.status, "ok")

        rec = client.start_investigation("owner/repo", issue="Fix auth failure")
        self.assertEqual(rec.id, "inv-1")

        done = client.wait_for_investigation("inv-1", poll_interval=0.01, timeout=2.0)
        self.assertEqual(done.status, "completed")

        report = client.get_report("inv-1")
        self.assertEqual(report.executive_summary, "All checks passed")

        cancelled = client.cancel_investigation("inv-1")
        self.assertEqual(cancelled.status, "cancelled")

    def test_error_mapping(self) -> None:
        unauth = BugHunterClient(f"http://127.0.0.1:{self.port}")
        with self.assertRaises(AuthenticationError):
            unauth.get_status("inv-1")

        client = BugHunterClient(f"http://127.0.0.1:{self.port}", api_key="secret-key")
        with self.assertRaises(NotFoundError):
            client.get_status("missing")


if __name__ == "__main__":
    unittest.main()
