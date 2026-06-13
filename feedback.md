# Review Feedback

Two blocking issues: agent message content is silently dropped from restored history, and conversations are not sorted newest-first as required by the plan.

## Findings

`issue: agent messages silently dropped from restored history`

Comment: In `src/main.rs` (ResumePicker `Select` branch), `ConversationMessage::AgentMessage` is matched and returns `None`, meaning only user messages are preserved on resume. The plan explicitly requires "both sides of the conversation can be reconstructed." The root cause is structural: `ConversationMessage::AgentMessage` in `src/dust/types.rs:95-101` has only `s_id` and `parent_message_id` — no `content` field. To fix, add a `content: Option<String>` field to the `AgentMessage` variant (the Dust API includes it in the conversation response) and include non-`None` content in the messages vec.

`issue: conversations not sorted newest-first`

Comment: `DustClient::list_conversations()` (`src/dust/client.rs:312-338`) returns `body.conversations` in API order with no sort step. The plan requires reverse chronological order by `updated` (or `created` as fallback). Add `.sort_by(|a, b| b.updated.unwrap_or(b.created).cmp(&a.updated.unwrap_or(a.created)))` before returning, or sort in `set_resume_conversations()`.

`issue: system confirmation message missing conversation title`

Comment: `app.restore_conversation()` (`src/app.rs:131`) pushes `"Resumed conversation"` but the plan specifies `"Resumed conversation: {title}"`. The conversation title is available in the `ConversationSummary` at picker select time (`filtered.get(selected)`) but is not threaded through to `restore_conversation()`. Either extend the signature to accept `title: Option<&str>` or push the system message at the call site in `main.rs` where the title is still in scope.

## Strengths

`praise: clean PickerState mirror`

Comment: `ResumePickerState` is a faithful structural mirror of `PickerState`, and `handle_picker_key` is reused without duplication. The early-return guard in `render_resume_picker` keeps the render path clean.

`praise: comprehensive unit test coverage`

Comment: All ten plan-specified unit tests are present and well-scoped — deserialization, filtering, selection wrap, restore behavior, and scroll reset each have a dedicated test. All 199 tests pass; clippy is clean.

`praise: format_relative_time implementation`

Comment: Inline helper in `src/ui/picker.rs` covers the full range (just now → minutes → hours → days), handles negative elapsed correctly (returns "just now"), and uses the `#[allow(clippy::cast_possible_truncation)]` annotation precisely rather than suppressing broader lints.

## Residual Risk

`suggestion: filter logic duplicated in move_selection`

Comment: `resume_picker_move_selection()` (`src/app.rs:95-119`) re-implements the same filter predicate as `resume_filtered_conversations()`. If filtering logic changes, both need updating. Replace the count computation with `self.resume_filtered_conversations().len()` to stay DRY.

`suggestion: untitled conversations invisible when filter is active`

Comment: `resume_filtered_conversations()` uses `is_some_and(...)` which excludes `title: None` conversations when any filter text is typed. The plan shows "(untitled)" as the display label, implying they remain visible and unfilterable rather than hidden. Decide and document the intended behavior; if they should always show, add a special case for `title.is_none()`.
