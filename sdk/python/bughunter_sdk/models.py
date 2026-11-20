from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any


@dataclass
class InvestigationRecord:
    id: str
    status: str
    request: dict[str, Any]
    result: dict[str, Any] | None = None
    error: str | None = None
    created_at: str = ""
    updated_at: str = ""

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "InvestigationRecord":
        return cls(
            id=data["id"],
            status=data["status"],
            request=data.get("request", {}),
            result=data.get("result"),
            error=data.get("error"),
            created_at=data.get("created_at", ""),
            updated_at=data.get("updated_at", ""),
        )


@dataclass
class Report:
    investigation_id: str
    title: str
    executive_summary: str
    workflow: str
    status: str
    findings: list[str] = field(default_factory=list)
    recommendations: list[str] = field(default_factory=list)
    generated_at: str = ""

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "Report":
        return cls(
            investigation_id=data["investigation_id"],
            title=data["title"],
            executive_summary=data["executive_summary"],
            workflow=data["workflow"],
            status=data["status"],
            findings=data.get("findings", []),
            recommendations=data.get("recommendations", []),
            generated_at=data.get("generated_at", ""),
        )


@dataclass
class HealthStatus:
    status: str
    version: str
    uptime_seconds: int
    active_investigations: int
    total_investigations: int

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "HealthStatus":
        return cls(
            status=data["status"],
            version=data["version"],
            uptime_seconds=data.get("uptime_seconds", 0),
            active_investigations=data.get("active_investigations", 0),
            total_investigations=data.get("total_investigations", 0),
        )
