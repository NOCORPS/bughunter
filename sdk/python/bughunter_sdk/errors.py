from __future__ import annotations


class BugHunterError(Exception):
    """Base error for all BugHunter SDK operations."""

    def __init__(self, message: str, code: str = "INTERNAL_ERROR", status_code: int | None = None):
        super().__init__(message)
        self.code = code
        self.status_code = status_code


class AuthenticationError(BugHunterError):
    pass


class AuthorizationError(BugHunterError):
    pass


class ValidationError(BugHunterError):
    pass


class NotFoundError(BugHunterError):
    pass


class RateLimitError(BugHunterError):
    pass


class TimeoutError(BugHunterError):
    pass
