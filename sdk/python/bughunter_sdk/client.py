from __future__ import annotations

import asyncio
import json
import time
import urllib.error
import urllib.request
from typing import Any

from .errors import (
    AuthenticationError,
    AuthorizationError,
    BugHunterError,
    NotFoundError,
    RateLimitError,
    TimeoutError,
    ValidationError,
)
from .models import HealthStatus, InvestigationRecord, Report


def _raise_for_status(status_code: int, body: str) -> None:
    try:
        payload = json.loads(body) if body else {}
    except json.JSONDecodeError:
        payload = {"message": body}

    err_obj = payload.get("error", {}) if isinstance(payload, dict) else {}
    code = err_obj.get("code", "API_ERROR") if isinstance(err_obj, dict) else "API_ERROR"
    msg = err_obj.get("message", body) if isinstance(err_obj, dict) else body

    mapping = {
        400: ValidationError,
        401: AuthenticationError,
        403: AuthorizationError,
        404: NotFoundError,
        422: ValidationError,
        429: RateLimitError,
    }
    exc_cls = mapping.get(status_code, BugHunterError)
    raise exc_cls(msg, code=code, status_code=status_code)


class BugHunterClient:
    """Synchronous Python SDK client for BugHunter REST API v1."""

    def __init__(self, base_url: str, api_key: str | None = None, timeout: float = 30.0):
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key
        self.timeout = timeout

    def _request(self, method: str, path: str, payload: dict[str, Any] | None = None) -> Any:
        url = f"{self.base_url}{path}"
        data = json.dumps(payload).encode("utf-8") if payload is not None else None
        headers = {"Content-Type": "application/json", "Accept": "application/json"}
        if self.api_key:
            headers["Authorization"] = f"Bearer {self.api_key}"

        req = urllib.request.Request(url, data=data, headers=headers, method=method)
        try:
            with urllib.request.urlopen(req, timeout=self.timeout) as resp:
                raw = resp.read().decode("utf-8")
                return json.loads(raw) if raw else {}
        except urllib.error.HTTPError as exc:
            body = exc.read().decode("utf-8", errors="replace")
            _raise_for_status(exc.code, body)
        except urllib.error.URLError as exc:
            raise BugHunterError(f"connection error: {exc.reason}", code="CONNECTION_ERROR") from exc

    def start_investigation(
        self,
        repository: str,
        issue: str | None = None,
        *,
        files: list[str] | None = None,
        verification_required: bool = True,
        provider_hint: str | None = None,
        model_hint: str | None = None,
    ) -> InvestigationRecord:
        payload: dict[str, Any] = {
            "repository": repository,
            "verification_required": verification_required,
        }
        if issue is not None:
            payload["issue"] = issue
        if files is not None:
            payload["files"] = files
        if provider_hint is not None:
            payload["provider_hint"] = provider_hint
        if model_hint is not None:
            payload["model_hint"] = model_hint
        data = self._request("POST", "/api/v1/investigations", payload)
        return InvestigationRecord.from_dict(data)

    def get_status(self, investigation_id: str) -> InvestigationRecord:
        data = self._request("GET", f"/api/v1/investigations/{investigation_id}")
        return InvestigationRecord.from_dict(data)

    def cancel_investigation(self, investigation_id: str) -> InvestigationRecord:
        data = self._request("POST", f"/api/v1/investigations/{investigation_id}/cancel", {})
        return InvestigationRecord.from_dict(data)

    def get_report(self, investigation_id: str) -> Report:
        data = self._request("GET", f"/api/v1/investigations/{investigation_id}/report")
        return Report.from_dict(data)

    def wait_for_investigation(
        self,
        investigation_id: str,
        poll_interval: float = 0.25,
        timeout: float = 60.0,
    ) -> InvestigationRecord:
        deadline = time.monotonic() + timeout
        while True:
            record = self.get_status(investigation_id)
            if record.status == "completed":
                return record
            if record.status == "failed":
                raise BugHunterError(record.error or "investigation failed", code="INVESTIGATION_FAILED")
            if record.status == "cancelled":
                raise BugHunterError("investigation cancelled", code="CANCELLED")
            if time.monotonic() >= deadline:
                raise TimeoutError(f"timed out waiting for investigation {investigation_id}", code="TIMEOUT")
            time.sleep(poll_interval)

    def analyze_repository(self, repository: str, files: list[str] | None = None) -> dict[str, Any]:
        return self._request(
            "POST",
            "/api/v1/repositories/analyze",
            {"repository": repository, "files": files or []},
        )

    def health(self) -> HealthStatus:
        return HealthStatus.from_dict(self._request("GET", "/api/v1/health"))


class AsyncBugHunterClient:
    """Async wrapper around BugHunterClient using asyncio.to_thread."""

    def __init__(self, base_url: str, api_key: str | None = None, timeout: float = 30.0):
        self._sync = BugHunterClient(base_url, api_key=api_key, timeout=timeout)

    async def start_investigation(self, *args: Any, **kwargs: Any) -> InvestigationRecord:
        return await asyncio.to_thread(self._sync.start_investigation, *args, **kwargs)

    async def get_status(self, investigation_id: str) -> InvestigationRecord:
        return await asyncio.to_thread(self._sync.get_status, investigation_id)

    async def get_report(self, investigation_id: str) -> Report:
        return await asyncio.to_thread(self._sync.get_report, investigation_id)

    async def cancel_investigation(self, investigation_id: str) -> InvestigationRecord:
        return await asyncio.to_thread(self._sync.cancel_investigation, investigation_id)

    async def wait_for_investigation(
        self,
        investigation_id: str,
        poll_interval: float = 0.25,
        timeout: float = 60.0,
    ) -> InvestigationRecord:
        deadline = time.monotonic() + timeout
        while True:
            record = await self.get_status(investigation_id)
            if record.status == "completed":
                return record
            if record.status == "failed":
                raise BugHunterError(record.error or "investigation failed", code="INVESTIGATION_FAILED")
            if record.status == "cancelled":
                raise BugHunterError("investigation cancelled", code="CANCELLED")
            if time.monotonic() >= deadline:
                raise TimeoutError(f"timed out waiting for investigation {investigation_id}", code="TIMEOUT")
            await asyncio.sleep(poll_interval)
