# dust — Dust API Client

HTTP client for the Dust platform API. Handles conversations, messages, agent listing, and Server-Sent Event streaming.

## Files

| File | Purpose |
|------|---------|
| `mod.rs` | Module exports |
| `client.rs` | `DustClient` struct — all HTTP interactions with Dust (create conversation, post message, list agents, SSE streaming) |
| `stream.rs` | `EventStream` — parses raw SSE bytes into typed `StreamEvent` variants |
| `types.rs` | Request/response DTOs (serde). All API types live here. |

## API Endpoints Used

| Method | Path | Function |
|--------|------|----------|
| POST | `/api/v1/w/{workspace}/assistant/conversations` | `create_conversation()` |
| POST | `/api/v1/w/{workspace}/assistant/conversations/{id}/messages` | `post_message()` |
| GET | `/api/v1/w/{workspace}/assistant/conversations/{id}` | `get_conversation()` (poll for agent reply) |
| GET | `/api/sse/v1/w/{workspace}/assistant/conversations/{id}/messages/{id}/events` | `stream_events()` |
| GET | `/api/v1/w/{workspace}/assistant/agent_configurations?view=list` | `list_agents()` |

## Data Flow: `send_message_flow()`

This is the main orchestration method — called from the TUI event loop:

1. Creates conversation (first message) or posts to existing one
2. Sends `DustEvent::ConversationCreated` via MPSC channel
3. Polls `get_conversation()` up to 100 times (300ms apart) waiting for agent message ID
4. Opens SSE stream for the agent message
5. Forwards `DustEvent::Token(text)` for each `generation_tokens` event
6. Sends `DustEvent::Complete` on `agent_message_success` or stream end

## SSE Parsing

The Dust API sends two SSE formats:
- **Direct**: `data: {"type":"generation_tokens","text":"...","classification":"tokens"}`
- **Enveloped**: `data: {"eventId":"...","data":{"type":"generation_tokens",...}}`

`stream.rs` handles both via `DustSseEnvelope` — tries envelope first, falls back to direct parsing.

## Key Patterns

- **HTTP headers**: Must match the official Dust CLI — `User-Agent: Dust CLI`, `X-Dust-CLI-Version: 0.4.5` (see ADR-0006)
- **Conversation titles**: Prefixed with `"CLI Question: "` + first 30 chars of message (see ADR-0007)
- **Region routing**: `europe-west1` -> `https://eu.dust.tt`, everything else -> `https://dust.tt`
- **Agent resolution**: env `OXIDE_AGENT_ID` > config file `agent_id` > default `"dust"`
- **Message context**: Always includes `origin: "cli"`, timezone, username, optional email/full_name
- **Error handling**: Uses `anyhow` throughout, structured `tracing` logs at debug/error levels

## Constants

- Default agent: `dust`
- Default base URL: `https://dust.tt`
- Default visibility: `unlisted`
- Default origin: `cli`
- Poll attempts: 100 (at 300ms intervals = 30s timeout)
