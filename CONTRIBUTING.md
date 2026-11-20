# Contributing to BugHunter

Thank you for contributing to BugHunter!

## Development Setup

1. Clone the repository:
   ```bash
   git clone https://github.com/NOCORPS/bughunter.git
   cd bughunter
   ```
2. Run the full test suite (Rust + Python SDK):
   ```bash
   make test
   ```
3. Optional: Install the `bughunter` CLI locally:
   ```bash
   make install-cli
   ```

## Architectural Rules

- **Single Debugging Engine**: All integrations (CLI, MCP, A2A, REST API, SDKs, Agent Adapters) must normalize requests into `DebuggingRequest` and delegate to `BugHunterCore` (`crates/bughunter-core`). Never duplicate debugging logic in an integration crate.
- **Unified Security**: Every integration must enforce `SecurityPolicy` (authentication, permissions, rate limiting, sandboxing, secret redaction, audit logging).
- **Isolated Plugins**: Third-party plugins must run out-of-process via `SubprocessPluginRunner` (`crates/bughunter-plugins`), never inside the core process.
- **Deterministic Tests**: Use local mock servers for provider and protocol tests so `make test` passes offline without live API keys.

## Submitting Changes

1. Create a feature branch from `main`.
2. Ensure `make test` passes with zero warnings/failures.
3. Update documentation in `docs/` or `README.md` if public APIs, CLI flags, or providers change.
4. Open a Pull Request against `main`.
