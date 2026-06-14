# Review Feedback

Phases 1–3 are complete and solid; Phase 4 tools plumbing is wired correctly; Phase 5 has the core loop working but 8 clippy errors block compilation and two plan deliverables are missing.

## Findings

`issue: 8 clippy errors prevent compilation under repo's strict lint policy`

Comment: `nix develop --command cargo clippy` fails with 8 errors that are `deny`-level under `Cargo.toml`'s `[lints.clippy]`:
- `src/mcp/process.rs:24` — `unused async: McpProcess::spawn()` has no await points (uses `std::process::Command` synchronously). Remove `async` or switch to `tokio::process::Command` with an await.
- `src/mcp/bash.rs:26` — `Duration::from_secs(60)` preferred over seconds (clippy `duration_lit_use_secs`)
- `src/mcp/client.rs:29`, `src/mcp/process.rs:55,83`, `src/mcp/mod.rs:67` — `variables can be used directly in format! string` (use `{name}` not `"{}", name`)
- `src/ui/tool_approval.rs:86,92,96` — same format string issue plus `this if statement can be collapsed`
- `src/mcp/types.rs:23` — `you can implement Eq` on `McpTool` (derives `PartialEq` but not `Eq`)
- `src/config.rs:24,61` — `this if statement can be collapsed`
The pre-commit hook runs `cargo clippy`, so these block every commit on this branch.

`issue: auto_approve is parsed but never consulted — opt-in fast path is dead`

Comment: `McpConfig::auto_approve` is correctly parsed and stored in `src/config.rs:24`. It is referenced only in test fixture construction (`src/mcp/mod.rs:107,127`) and never read in the runtime path. The `DustEvent::ToolUse` handler in `src/main.rs:395-398` unconditionally calls `app.enter_tool_approval(tool_call)`. A user who sets `auto_approve = true` in their config still sees the modal. The plan's acceptance criterion ("with `auto_approve = true`, the modal is skipped entirely") is not met. Fix: before calling `enter_tool_approval`, check `config.mcp().auto_approve` and if true, spawn the tool execution task directly instead.

`issue: handler.rs not updated — Action::ApproveTool / Action::DenyTool missing`

Comment: Phase 5 deliverables explicitly require `src/handler.rs` — in `ToolApproval` mode: `y` → `Action::ApproveTool`, `n` → `Action::DenyTool`, `Esc` → deny. Instead, the key dispatch is implemented directly in the `AppMode::ToolApproval` arm of the `main.rs` event loop (`src/main.rs:240-298`). This means: (1) the `Action` enum does not model tool approval, (2) handler tests cannot cover this path, (3) any future refactor of the event loop has to know about this hidden exception. Move the `y`/`n`/`Esc` handling to `handler.rs` via new `Action` variants.

`issue: no SSE test for tool_use parsing (Phase 4 acceptance criterion)`

Comment: Phase 4 requires "SSE fixture with `tool_use` event parses correctly into `ToolCall`". The `extract_tool_use_from_action` method (`src/dust/types.rs:147-165`) does the parsing but has no unit test. A test similar to `parses_generation_tokens_event` in `src/dust/stream.rs` should construct a fixture `AgentActionSuccess` payload with `type: "tool_use"`, `id: "toolu_abc"`, `name: "bash"`, `input: { "command": "echo hi" }` and assert it deserializes into the expected `ToolCall`.

`issue: no mode-transition tests (Phase 5 acceptance criterion)`

Comment: Phase 5 requires "unit tests: mode transitions, key mappings, auto_approve bypass". None exist. `enter_tool_approval`, `exit_tool_approval`, and `current_tool_call` in `src/app.rs:384-398` have no coverage. At minimum: test that `enter_tool_approval` transitions from `Chat` to `ToolApproval`, that `current_tool_call` returns the stored call, and that `exit_tool_approval` returns to `Chat` with `None`.

## Strengths

`praise: tools are correctly threaded into the Dust message POST`

Comment: `src/main.rs:460-463` locks `mcp_manager`, calls `list_tools()`, and passes the result into `send_message_flow`. `src/dust/client.rs:593,617` passes the tools list to `message_body_with_tools()` with an `if tools.is_empty() { None }` guard, keeping the POST body clean when no MCP servers are configured. This is the most critical path for end-to-end functionality and it is correct.

`praise: Drop impl is fixed — McpProcess uses start_kill()`

Comment: `src/mcp/process.rs:17-20` now calls `self.child.start_kill()` in `Drop`, which is the correct non-async sync-compatible kill for `tokio::process::Child`. Avoids the earlier `kill()` bug where the future was silently dropped.

`praise: tool_approval.rs renders cleanly with correct layout`

Comment: `src/ui/tool_approval.rs` follows the picker popup pattern exactly (centered 70×70 via `centered_rect`, `Clear` to erase background, `Block` with bordered title). The input formatting via `format_input()` handles object, scalar, and fallback cases. The prompts line shows `[y] approve`, `[n] deny`, `[Esc] cancel` with appropriate colours. Wired correctly at `src/main.rs:135`.

`praise: streaming architecture matches the plan's SSE-hold design`

Comment: The plan states "The SSE stream is held open but not consumed while Oxide waits for approval." The implementation achieves this correctly: `send_message_flow` runs in a spawned task that blocks on `stream.next_event().await` after emitting `DustEvent::ToolUse`. The approval and tool execution happen on the main event loop via a separate `tokio::spawn`. When the tool result is submitted to Dust, Dust continues the SSE stream — which the original spawned task is still blocked on. No second stream or re-polling is needed, and the implementation is consistent with this design.

`praise: denial path submits a structured error result`

Comment: The `n`/Esc handler at `src/main.rs:271-293` constructs `ToolResult { is_error: true, content: "denied by user" }` and submits it to `submit_tool_result`. This is correct — silent discard would hang the agent waiting for a response. The structured error lets the Dust agent recover and respond to the user.

## Residual Risk

`suggestion: McpProcess::spawn mixes sync spawn with async signature`

Comment: Removing `async` from `McpProcess::spawn` (to fix the clippy error) is straightforward since `std::process::Command::spawn` is synchronous. But verify no call-site assumes the function is async — currently called with `.await` in `McpClient::connect` at `src/mcp/client.rs:17`. Either keep `async` and switch to `tokio::process::Command::spawn()` with an await, or remove `async` and remove the `.await` at all call sites.

`suggestion: no test for McpClient tool dispatch routing`

Comment: Phase 3 requires "unit tests: JSON-RPC serialization/deserialization, tool dispatch routing". The `McpManager` tests cover the builtin bash path. There are no tests for external server dispatch (routing by tool name across multiple `McpClient` instances). A mock MCP server (echo JSON-RPC responses over stdin/stdout) would cover both the serialization and dispatch paths.

`suggestion: tool_approval.rs uses emoji — check terminal rendering`

Comment: `src/ui/tool_approval.rs:39` renders `⚙ ` as a status icon. Emoji rendering width varies across terminal emulators (1 vs 2 cells). If the tool name text bleeds into the separator column, consider replacing with ASCII (e.g. `[*]`) until a tmux render is confirmed.
