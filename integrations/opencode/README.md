# OpenCode Integration

## MCP Integration (Recommended)

Add the `bughunter` MCP server entry from `opencode.json` to your project's `opencode.json` or `~/.config/opencode/opencode.json`.

Exposed tools:
- `bughunter.investigate`
- `bughunter.analyze_repository`
- `bughunter.reproduce`
- `bughunter.diagnose`
- `bughunter.generate_patch`
- `bughunter.verify`
- `bughunter.get_report`
- `bughunter.get_status`

## CLI Fallback

If MCP is disabled, invoke BugHunter via shell tool with `--json`:

```bash
bughunter investigate --repository . --issue "Describe the bug" --verify --json
```
