# Review Feedback

The MCP infrastructure compiles and tests pass, but Phase 3 (tool-call UX and streaming integration) is a stub — the core loop that makes MCP useful does not run.

## Findings

`issue: McpManager is never constructed — entire MCP stack is dead code`

Comment: `src/main.rs` declares `mod mcp;` but never calls `McpManager::init()`. As a result, every type and function in `src/mcp/` is unreachable at runtime. Clippy confirms this: `struct McpManager is never constructed`, `struct BashTool is never constructed`, `struct McpClient is never constructed`, etc. The plan requires `McpManager::init(config.mcp())` to be called at TUI startup before the event loop (`src/main.rs`, after `Config::load()`).

`issue: tool approval handler is a stub with dead variables`

Comment: `src/main.rs:229-251` handles `AppMode::ToolApproval` key events but does nothing on approval. `input_json` and `tool_use_id` are cloned and immediately dropped — clippy flags both as unused variables. The plan requires: approve → `McpManager::call_tool()` → set `tool_use_id` on the result → `DustClient::submit_tool_result()` → resume streaming. None of this is wired up. The comment `// Tool execution will be handled after we can initialize McpManager` confirms this is intentionally deferred.

`issue: ToolApproval mode is never entered from stream events`

Comment: The plan's data flow is: `StreamEvent::AgentActionSuccess { type: "tool_use" }` → `extract_tool_use()` → `app.enter_tool_approval(tool_call)`. The `extract_tool_use()` method exists on `StreamEvent` but nothing in `main.rs` calls it. The streaming handler in `send_message_flow` never triggers the approval mode, so even with a properly initialized `McpManager`, the UX gate never fires.

`issue: tool_use_id is always empty string in ToolResult`

Comment: `McpManager::call_tool()` (`src/mcp/mod.rs:97,107`) constructs `ToolResult { tool_use_id: String::new(), … }`. The actual ID from the Dust event is never threaded through. When `submit_tool_result()` posts to `…/tool_results`, the `tool_use_id` field will be `""`, which will either be rejected by the API or silently misroute the result.

`issue: auto_approve is never checked`

Comment: `McpConfig::auto_approve` is parsed and stored but never read (clippy: `fields auto_approve and servers are never read` since `Config::mcp()` itself is never called). The plan's fast path — skip the modal when `auto_approve = true` — is not implemented.

`issue: clippy deny violations will block the pre-commit hook`

Comment: The project's `Cargo.toml` sets `[lints.clippy] all = "deny"` and `pedantic = "deny"`. Running strict clippy produces ~30 warnings, many of which are `deny`-level in the repo's policy: unused import `std::io::BufReader` (`src/mcp/process.rs:1`), unused import `ToolApproval` (`src/mcp/mod.rs`), `this could be a const fn` in `src/mcp/bash.rs`, `unnecessary structure name repetition` in multiple MCP files, `this continue expression is redundant` in `src/mcp/mod.rs`. These will fire as errors in CI/pre-commit.

`issue: handler.rs not updated — plan required Action::ApproveTool / Action::DenyTool`

Comment: The plan explicitly states: "`src/handler.rs` — in `ToolApproval` mode: `y` → `Action::ApproveTool`, `n` → `Action::DenyTool`, `Esc` → deny". The key handling was placed directly in the `main.rs` event loop instead, bypassing the `Action` enum and `handle_key_event()` abstraction used by all other modes. This is architecturally inconsistent and makes the handler untestable.

## Strengths

`praise: config parsing is well-tested and complete`

Comment: `src/config.rs` adds three focused tests (`parses_mcp_builtin_bash_server`, `parses_mcp_external_server`, `defaults_mcp_config_when_missing`) that cover the happy path, external servers with args, and the zero-config default. The `#[serde(default)]` annotations are correct throughout.

`praise: BashTool has correct behavior and good test coverage`

Comment: `src/mcp/bash.rs` tests echo, non-zero exit, stderr capture, and empty output. The stdout+stderr concatenation and the 60-second timeout are both sensible defaults. The `is_error` flag correctly reflects `!output.status.success()`.

`praise: McpProcess uses async I/O correctly`

Comment: `src/mcp/process.rs` uses `tokio::io::BufWriter`/`BufReader` throughout (not std blocking I/O) and flushes before reading the response. The JSON-RPC framing (newline-delimited) is standard for MCP servers.

`praise: extract_tool_use() is clean and non-panicking`

Comment: `StreamEvent::extract_tool_use()` (`src/dust/types.rs:146-155`) uses `serde_json::from_value(…).ok()` to silently discard malformed tool-use events rather than panicking or propagating errors. Good default for a streaming context.

## Residual Risk

`suggestion: ToolApproval UI is not rendered`

Comment: There is no arm for `AppMode::ToolApproval` in the UI render path. The user will see a blank or stale screen while the app waits for `y`/`n`. The plan references a "tool-call block with [y/n] prompt" — add a `ui/tool_approval.rs` widget following the existing popup pattern (see agent picker at `src/ui/picker.rs`) before this is user-visible.

`suggestion: McpProcess child is leaked on drop`

Comment: `McpProcess` holds a `Child` but has no `Drop` impl to kill the subprocess. When `McpClient` or `McpManager` is dropped, the child process lingers. Add `impl Drop for McpProcess { fn drop(&mut self) { let _ = self.child.kill(); } }` or use a `KillOnDrop` wrapper.

`suggestion: no test for ToolApproval mode transitions`

Comment: The plan's acceptance criteria include: "pressing `y` executes the tool and resumes streaming", "pressing `n` submits a denial result", "auto_approve bypasses the modal". None of these are tested. Add unit tests for `app.enter_tool_approval()` / `app.exit_tool_approval()` and for the key-dispatch logic once it moves to `handler.rs`.
