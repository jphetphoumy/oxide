# ADR-0010: Mandatory Tool Approval Gate with Full Call Visibility

**Date**: 2026-06-15
**Status**: Accepted
**Supersedes**: ADR-0009 (tool approval behavior)

## Context

Previously, builtin tools (`oxide_bash`, `oxide_skill`, `oxide_agent`) were automatically approved without user interaction, making it impossible to prevent accidental or malicious command execution. Users could not review what commands would be executed before they ran.

This creates two security risks:
1. **No visibility** — users cannot see the exact command/tool call before execution
2. **No control** — builtin tools bypass the approval gate entirely

## Decision

All tool calls—builtin and external—now require explicit user approval by default. The approval gate shows the **full tool call details** (tool name and arguments) so users can review before executing.

### Approval Flow

1. **Agent requests tool execution** → Dust server sends tool call to oxide
2. **Oxide intercepts and shows approval UI** with full tool details:
   ```
   Approve tool call?
   
   Tool: oxide_bash
   Input: {"command": "ls -la /home/jphetphoumy"}
   
   [y]es  [n]o
   ```
3. **User reviews and presses key**:
   - `y` or `Enter` → approve and execute
   - `n` or `Esc` → deny (agent gets error)
4. **Result sent to agent** → either success or denial error

### Auto-approval Policy

**Safe tools (always auto-approved):**
- `oxide_skill` — read-only file access, no execution, no side effects

**Dangerous tools (require approval by default):**
- `oxide_bash` — arbitrary command execution
- `oxide_agent` — spawns subagent conversations
- External MCP tools — user-configured, unknown safety profile

Users can globally enable auto-approval in config:

```toml
[mcp]
auto_approve = true  # Skip approval gate for dangerous tools
```

When `auto_approve = true`, dangerous tools bypass the approval UI. This should only be used in trusted automation environments. Safe tools (`oxide_skill`) are always auto-approved regardless of this setting.

### Tool Details in Approval UI

The approval UI displays:
- **Tool name** — identifies which tool is being called
- **Full input JSON** — shows all arguments and parameters

This enables users to catch:
- Unintended commands (`rm -rf /` vs. `ls`)
- Suspicious arguments (API keys in plaintext)
- Tools the agent shouldn't have access to

## Consequences

- **Zero automatic execution** — all tool calls require human review unless `auto_approve = true`
- **Breaking change** — agents requesting tool execution will now pause at the approval gate
- **Security improvement** — users can veto dangerous commands before they run
- **Config controls approval** — `auto_approve` applies uniformly to all tools (builtin and external)
- **No special cases** — builtin tools (`oxide_bash`, `oxide_skill`) are treated like any other tool
- **Tool visibility** — the full JSON input shows exactly what the agent is trying to do
