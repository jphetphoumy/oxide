# Review Feedback

No blocking findings — all three issues from the previous review are correctly resolved.

## Strengths

`praise: agent messages restored from content field`

Comment: `ConversationMessage::AgentMessage` in `src/dust/types.rs:428-429` now has `content: Option<String>` with `#[serde(default)]`. The select handler in `main.rs:690-692` extracts it with `content.as_ref().map(|c| ("agent".to_string(), c.clone()))`, correctly skipping `None` (in-progress messages). The `find_agent_message()` helper is unaffected — the `..` ignore pattern at `client.rs:362` handles the new field cleanly.

`praise: sort newest-first applied in the right place`

Comment: The sort is placed in `list_conversations()` (`src/dust/client.rs:337-343`) rather than in the UI layer, which means the ordering guarantee is at the data boundary and doesn't depend on call-site discipline. The `updated.unwrap_or(created)` fallback correctly handles conversations the API hasn't updated yet.

`praise: title threaded through to system message`

Comment: `restore_conversation()` now takes `title: Option<&str>` and formats `"Resumed conversation: {title_str}"` with `"(untitled)"` as fallback. The title is captured at selection time from `conv.title.clone()` before the async spawn, which avoids lifetime issues.

`praise: move_selection deduplication`

Comment: `resume_picker_move_selection()` (`src/app.rs:97`) now delegates count to `self.resume_filtered_conversations().len()` — the filtering predicate lives in one place.

`praise: untitled conversations remain visible during filter`

Comment: `resume_filtered_conversations()` (`src/app.rs:75-79`) now passes untitled conversations through with `c.title.is_none() || c.title.as_ref().is_some_and(...)`, consistent with the display label "(untitled)" in the picker rows.

## Residual Risk

`suggestion: no test for sort order`

Comment: `list_conversations()` sorts in-place but no unit test verifies the newest-first ordering. The method is `async` (can't be called in a sync test directly), but the sort predicate can be extracted and tested independently, or the test can construct two `ConversationSummary` values and assert post-sort order. Without this, a regression in the comparator would go undetected.

`suggestion: no test for untitled-conversation filter passthrough`

Comment: The fix to `resume_filtered_conversations()` — keeping untitled conversations visible when a filter is active — has no corresponding test. Add a case: one titled conversation that does not match the filter, one untitled; assert the untitled one appears in the filtered results.

`suggestion: no test for restore_conversation with None title`

Comment: The "(untitled)" fallback path in `restore_conversation()` is not covered. `restore_conversation_sets_conversation_id` passes `None` but does not assert the content of the system message. Add an assertion that the system message reads `"Resumed conversation: (untitled)"` when `title` is `None`.
