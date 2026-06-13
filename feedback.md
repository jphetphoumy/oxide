# Review Feedback

The MCP skeleton is wired end-to-end but has four blocking issues: a broken Drop impl that leaks processes, 13 clippy errors that prevent strict-lint compilation, no UI for the approval prompt, and a missing second-stream after tool result submission.

## Findings

`issue: Drop impl on McpProcess is a no-op — child processes are never killed`

Comment: `src/mcp/process.rs:17-21` implements `Drop` for `McpProcess` with `let _ = self.child.kill();`. `tokio::process::Child::kill()` returns a future — it does not do anything when assigned to `_`. Clippy flags this as `non-binding let on a future`. The child process is leaked every time `McpProcess` is dropped (on error path, on shutdown). Fix: use `std::process::Child` for blocking kill in Drop, or store a `tokio::process::Child` and call the sync `start_kill()` instead: `let _ = self.child.start_kill();`.

`issue: 13 clippy errors prevent compilation under the repo's strict lint policy`

Comment: `nix develop --command cargo clippy` (which applies the `Cargo.toml` deny rules) fails with 13 errors:
- `src/mcp/process.rs:19` — `non-binding let on a future` (see above)
- `src/mcp/process.rs:24` — `unused async: spawn()` has no await points (uses sync `std::process::Command`)
- `src/mcp/mod.rs:95` — `redundant continue` in the `Err(_) => continue` arm
- `src/mcp/mod.rs:67,99` — `variables can be used directly in format! string`
- `src/mcp/client.rs:29`, `src/mcp/bash.rs:26` — `variables can be used directly in format! string`
- `src/mcp/types.rs:23` — `you can implement Eq` (derives `PartialEq` on `McpTool`)
- `src/config.rs:24,61` — `this if statement can be collapsed` (nested `if let`)
- `src/main.rs:249,274` — `redundant closure` (`.map(|s| s.to_string())`)
- `src/main.rs:256-257,286-287` — `this if statement can be collapsed`
The pre-commit hook runs `cargo clippy` which means these will block every commit.

`issue: no UI rendered in ToolApproval mode`

Comment: The render path in `src/main.rs:122-133` calls `render_layout`, `render_messages`, `render_input`, and `render_command_menu` unconditionally. For `AppMode::Picker` and `AppMode::ResumePicker` there are dedicated overlay renders. There is no `render_tool_approval` call. When the app enters `ToolApproval` mode, the user sees the last chat state with no indication that they need to press `y` or `n`, and keypresses (other than those two) are silently discarded. Add a centered modal (following the picker overlay pattern at `src/ui/picker.rs`) showing the tool name, input arguments, and `[y] approve  [n] deny` prompt.

`issue: no second stream after tool result submission — conversation stalls`

Comment: `send_message_flow()` in `src/dust/client.rs` opens one SSE stream for one agent message. When `AgentActionSuccess { type: "tool_use" }` is received, the stream continues but the Dust API will end that agent message's stream and create a new agent message once the tool result is submitted. The submitted result triggers a new `agent_message_id` in the conversation, but nothing in `main.rs` or `client.rs` polls for or streams that follow-up message. After the user approves a tool call, the conversation silently stops — no tokens, no completion event. The fix requires either: (a) looping back into `send_message_flow` after tool result submission to pick up the next agent message, or (b) making `send_message_flow` aware of tool calls and awaiting approval before continuing.

`issue: auto_approve is never consulted — modal always shown`

Comment: `McpConfig::auto_approve` is parsed correctly and stored, but the only references in `src/mcp/mod.rs:110,130` are in test fixture construction. The `DustEvent::ToolUse` handler in `src/main.rs:386-389` unconditionally calls `app.enter_tool_approval()`. When a user sets `auto_approve = true` in their config, the modal still appears. The check should be: if `config.mcp().auto_approve`, immediately spawn the tool execution task instead of entering approval mode.

`issue: handler.rs not updated — plan required Action::ApproveTool / Action::DenyTool`

Comment: The plan's Phase 3 spec explicitly requires adding `ApproveTool` and `DenyTool` to the `Action` enum in `src/handler.rs` and routing `y`/`n` through `handle_key_event`. Instead, the key logic is inlined directly in the `AppMode::ToolApproval` arm of `main.rs`. This makes the approval flow untestable (the handler tests in `handler.rs` cannot cover it) and breaks the layered architecture where `handler.rs` is the sole key→action translator.

## Strengths

`praise: McpManager::init() is correctly placed at startup`

Comment: `src/main.rs:102-106` initialises `McpManager` inside an `Arc<Mutex<>>` before the event loop, which is the right ownership shape for sharing across tokio spawns. Errors during server startup are surfaced as `io::Error::other`, consistent with how `Config::load` errors are handled in the same scope.

`praise: ToolUse → DustEvent → enter_tool_approval wiring is correct`

Comment: `src/dust/client.rs:410-418` catches `AgentActionSuccess` with `type: "tool_use"`, extracts the call via `extract_tool_use_from_action()`, and sends `DustEvent::ToolUse`. `src/main.rs:386-389` receives it and calls `app.enter_tool_approval(tool_call)`. The end-to-end event path from stream to app state is correctly threaded.

`praise: tool_use_id is threaded correctly on the approve path`

Comment: `src/main.rs:246,255` captures `tool_call.id` into `tool_use_id` before `exit_tool_approval()` drops the state, then sets `result.tool_use_id = tool_use_id` after the tool executes. This avoids the earlier stub's bug where the ID would have been an empty string.

`praise: denial result is well-formed`

Comment: The `n` / Esc handler at `src/main.rs:272-294` constructs a proper denial `ToolResult` with `is_error: true` and `content: "denied by user"` and submits it to Dust. Sending a structured denial (rather than silently dropping) is the correct approach — it lets the Dust agent recover gracefully.

`praise: McpProcess Drop impl added`

Comment: `src/mcp/process.rs:17-21` shows the author recognized child processes need cleanup on drop. The intent is correct even though the implementation is broken (see finding above). The struct was not left with a silent leak by omission.

## Residual Risk

`suggestion: no test for ToolApproval mode transitions`

Comment: `src/app.rs` has `enter_tool_approval`, `exit_tool_approval`, and `current_tool_call` but no unit tests cover them. A test asserting mode transitions (Chat → ToolApproval → Chat) and that `current_tool_call()` returns None after exit would catch regressions during the streaming-loop refactor needed to fix the second-stream issue.

`suggestion: McpClient has no timeout`

Comment: `src/mcp/process.rs` reads responses with `read_line` but there is no timeout. A hung external MCP server will stall the entire TUI event loop. Wrap the `read_line` call with `tokio::time::timeout` (same pattern as `BashTool::BASH_TIMEOUT`) or add a per-request deadline.

`suggestion: tools list is not passed to Dust when sending messages`

Comment: `src/dust/client.rs:517` calls `self.message_body_with_tools(message, agent_id, None)` — the `tools` parameter is always `None`. The `message_body_with_tools` plumbing exists, but the MCP tool list from `McpManager::list_tools()` is never injected into the outgoing message. Without advertising tools to Dust, the agent cannot know to emit `tool_use` actions. This may explain why the end-to-end flow would never trigger in practice.
