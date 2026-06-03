---
name: dust-api
description: Reference the Dust API swagger spec and official CLI implementation for parity when working on the oxide codebase. Use when adding or modifying Dust API calls, types, or streaming logic in src/dust/.
---

## When to use this

Use this skill when working on the oxide project and you need to:

- Add or modify a Dust API endpoint call
- Update request/response types in `src/dust/types.rs`
- Verify oxide's HTTP behavior matches the official Dust CLI
- Check the Dust API swagger spec for available endpoints, parameters, or response shapes
- Understand how the official CLI handles auth, streaming, conversations, or agent listing

## Setup: ensure the Dust repo is available

The Dust monorepo should live next to the oxide project. Before doing anything else, make sure it's available and up to date:

```bash
DUST_REPO="$(cd /home/jphetphoumy/Documents/Dev/dust 2>/dev/null && pwd || echo "")"

if [ -z "$DUST_REPO" ]; then
  echo "Cloning dust repo..."
  git clone git@github.com:dust-tt/dust.git /home/jphetphoumy/Documents/Dev/dust
  DUST_REPO="/home/jphetphoumy/Documents/Dev/dust"
else
  echo "Updating dust repo..."
  git -C "$DUST_REPO" pull --ff-only 2>/dev/null || echo "Pull skipped (dirty tree or no remote)"
fi
```

## Reference files

After setup, these are the key files to consult:

### Swagger / OpenAPI spec

The canonical API spec lives at:

```
$DUST_REPO/front/swagger.json
```

Use this to look up:
- Available endpoints and HTTP methods
- Request body schemas and required fields
- Response shapes and status codes
- Query parameters

There is also a generated swagger at `$DUST_REPO/front/public/swagger.json`.

### Official Dust CLI source

The official CLI is a TypeScript Ink app at:

```
$DUST_REPO/cli/dust-cli/src/
```

Key files for parity reference:

| File | What to check |
|------|---------------|
| `utils/dustClient.ts` | HTTP client setup, headers (`User-Agent`, `X-Dust-CLI-Version`), region routing, auth token injection |
| `utils/authService.ts` | Token refresh logic, expiry handling (30s skew), WorkOS integration |
| `utils/tokenStorage.ts` | What gets stored (access token, refresh token, workspace ID, region) |
| `ui/commands/Chat.tsx` | Conversation creation flow, message posting, streaming |
| `ui/commands/Auth.tsx` | Device code auth flow, workspace selection |
| `ui/components/AgentSelector.tsx` | Agent listing and selection |
| `ui/components/Conversation.tsx` | SSE streaming, token handling, message display |
| `ui/commands/chat/nonInteractive.ts` | Non-interactive chat flow (simpler reference for the API sequence) |

### SDK types

The JS SDK that the CLI uses is at:

```
$DUST_REPO/sdks/js/
```

This contains the canonical TypeScript types for API requests and responses.

## How to use this information

### When adding a new API endpoint to oxide

1. Read the swagger spec to find the endpoint path, method, and schemas
2. Check the official CLI to see how it calls that endpoint (headers, body format, error handling)
3. Implement in `src/dust/client.rs` following the existing patterns
4. Add serde types in `src/dust/types.rs` matching the swagger response schema
5. Run `cargo test` and `cargo clippy -- -W clippy::all -W clippy::pedantic -W clippy::nursery`

### When debugging API parity issues

1. Compare oxide's request headers against `dustClient.ts` (User-Agent, X-Dust-CLI-Version)
2. Compare request body serialization against the swagger spec
3. Check if the CLI handles any edge cases (retries, error codes, polling intervals)

### When updating types

1. Check `$DUST_REPO/front/swagger.json` for the current schema
2. Cross-reference with `$DUST_REPO/sdks/js/` for TypeScript type definitions
3. Update `src/dust/types.rs` serde structs to match

## Oxide's current API surface

For quick reference, oxide currently implements these Dust API calls (in `src/dust/client.rs`):

- `POST /api/v1/w/{wId}/assistant/conversations` — create conversation
- `POST /api/v1/w/{wId}/assistant/conversations/{cId}/messages` — post message
- `GET /api/v1/w/{wId}/assistant/conversations/{cId}` — poll for agent reply
- `GET /api/sse/v1/w/{wId}/assistant/conversations/{cId}/messages/{mId}/events` — SSE stream
- `GET /api/v1/w/{wId}/assistant/agent_configurations?view=list` — list agents
