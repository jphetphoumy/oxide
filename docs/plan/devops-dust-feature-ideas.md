# DevOps-Oriented Dust Feature Ideas For Oxide

## Goal

Identify Dust API capabilities that Oxide does not currently expose and rank the ones most likely to help a DevOps/SRE/platform engineer move faster from a terminal.

## Current Oxide Baseline

Oxide already supports:

- interactive chat with one selected agent
- agent switching via `/switch`
- conversation resume via `/resume`
- local skills injection
- MCP/subagent execution and tool approval for current streaming messages

Relevant current implementation points:

- `src/main.rs` runs a single chat event loop and only dispatches built-in slash commands for new/switch/resume
- `src/dust/client.rs` implements conversation create/post/get, SSE answer streaming, agent listing, MCP result submission, and action validation
- `src/dust/types.rs` only models the subset of Dust conversation/file/action schemas Oxide currently needs
- `src/ui/messages.rs` renders user, agent, system, subagent, and current approval states, but not attachment metadata, action history, or retryable blocked actions

## Dust Capability Gaps

The neighboring Dust repo exposes additional capabilities that Oxide does not currently use:

- file upload and attachment content fragments
- mention suggestions inside and outside conversations
- conversation-scoped toolset mutation
- blocked action listing and message retry
- answering structured user questions from agents
- message and conversation feedback APIs
- file download / view APIs

## Ranked Feature Ideas

### 1. Native file attachments for logs, manifests, runbooks, and incident artifacts

- Why it matters:
  DevOps work is artifact-heavy. The fastest workflow is attaching `kubectl describe`, Terraform plans, Helm values, YAML, screenshots, or compressed logs directly instead of pasting them into the prompt.
- Dust contracts:
  - `POST /api/v1/w/{wId}/files`
  - `POST /api/v1/w/{wId}/assistant/conversations/{cId}/content_fragments`
  - `POST /api/v1/w/{wId}/assistant/conversations` with `contentFragments`
  - `POST /api/v1/w/{wId}/assistant/conversations/{cId}/messages`
- Evidence:
  - Dust JS SDK attachment flow in `dust/sdks/js/src/high_level/stream.ts`
  - Dust CLI file uploader in `dust/cli/dust-cli/src/ui/components/FileUpload.tsx`
- Oxide gap:
  No slash command, state, UI, or Dust client path exists for attaching local files to a prompt.
- Complexity:
  Medium
- Value:
  Very high

### 2. Action timeline plus blocked-action recovery

- Why it matters:
  SRE workflows often fail on permissions, approvals, or external tool preconditions. Seeing which tool failed, why it blocked, and being able to retry only blocked actions is much more useful than just seeing the final answer stall.
- Dust contracts:
  - `GET /api/v1/w/{wId}/assistant/conversations/{cId}/actions/blocked` inferred from SDK and route files
  - `POST /api/v1/w/{wId}/assistant/conversations/{cId}/messages/{mId}/retry`
  - `POST /api/v1/w/{wId}/assistant/conversations/{cId}/messages/{mId}/validate-action`
- Evidence:
  - SDK methods `getBlockedActions`, `retryMessage`, `validateAction` in `dust/sdks/js/src/index.ts`
  - API routes under `dust/front-api/routes/v1/w/[wId]/assistant/conversations/[cId]/...`
- Oxide gap:
  Oxide can approve or deny the current approval prompt, but it does not persist an action history, list blocked actions after the fact, or retry blocked work.
- Complexity:
  Medium
- Value:
  High

### 3. Structured agent question answering

- Why it matters:
  If an agent pauses and asks the user to choose options or supply a structured answer, the terminal client should support that natively instead of forcing the user to switch to web.
- Dust contracts:
  - `POST /api/v1/w/{wId}/assistant/conversations/{cId}/messages/{mId}/answer-question`
- Evidence:
  - Swagger entry in `dust/front-api/public/swagger.json`
  - SDK method `answerUserQuestion` in `dust/sdks/js/src/index.ts`
- Oxide gap:
  `src/dust/types.rs` does not model this action shape, and the UI has no mode for multiple-choice or typed structured responses.
- Complexity:
  Medium to high
- Value:
  High for tool-heavy agents

### 4. In-conversation agent mentions and handoff commands

- Why it matters:
  During incidents, a platform engineer may want to pull a networking, database, or security agent into the same thread rather than abandoning context and starting over with `/switch`.
- Dust contracts:
  - `GET /api/v1/w/{wId}/assistant/mentions/suggestions`
  - `GET /api/v1/w/{wId}/assistant/conversations/{cId}/mentions/suggestions`
  - existing conversation/message payload `mentions`
- Evidence:
  - Swagger mention endpoints
  - SDK method around mention suggestions in `dust/sdks/js/src/index.ts`
- Oxide gap:
  Oxide only supports one active agent id for outgoing messages and does not surface mention discovery or multi-agent continuation flows.
- Complexity:
  Medium
- Value:
  Medium to high

### 5. Conversation-scoped toolset toggling

- Why it matters:
  A DevOps engineer often needs to add or remove AWS/Kubernetes/GitHub/internal tools per incident. Doing that without leaving the terminal makes Oxide more competitive with richer chat clients.
- Dust contracts:
  - `POST /api/v1/w/{wId}/assistant/conversations/{cId}/tools`
- Evidence:
  - SDK method `postConversationTools` in `dust/sdks/js/src/index.ts`
- Oxide gap:
  Oxide initializes client-side MCP once at startup, but it has no UX for mutating the toolset attached to an ongoing Dust conversation.
- Complexity:
  High
- Value:
  Medium to high

### 6. Artifact export and file download

- Why it matters:
  If an agent generates a config, report, or transformed file, a terminal-first user should be able to pull it down locally without copy/paste.
- Dust contracts:
  - `GET /api/w/{wId}/files/{fileId}?action=download`
  - `GET /api/w/{wId}/files/{fileId}?action=view&version=original|processed`
- Evidence:
  - SDK methods `downloadFile` and `getFileContent`
- Oxide gap:
  No file browser, no file id surfacing, no save/download workflow.
- Complexity:
  Medium
- Value:
  Medium

### 7. Feedback and retry ergonomics

- Why it matters:
  Lightweight retry and feedback help when iterating on incident prompts or validating new internal agents.
- Dust contracts:
  - `GET /api/v1/w/{wId}/assistant/conversations/{cId}/feedbacks`
  - `POST|DELETE /api/v1/w/{wId}/assistant/conversations/{cId}/messages/{mId}/feedbacks`
  - `POST /api/v1/w/{wId}/assistant/conversations/{cId}/messages/{mId}/retry`
- Oxide gap:
  No direct rating/retry controls after a message completes.
- Complexity:
  Low to medium
- Value:
  Medium

## Recommended First Feature

Start with **native file attachments**.

Reasoning:

- It has the clearest DevOps payoff immediately.
- The API path is well-defined and already mirrored in Dust's SDK.
- It fits Oxide's current chat mental model better than toolset mutation or structured question flows.
- It unlocks later work: downloaded artifacts, richer message rendering, and incident bundle workflows.

## Suggested First Implementation Plan

### Scope

Implement staged local file attachments for chat prompts, covering both new conversations and resumed conversations.

Target UX:

- `/attach <path>` stages a file
- `/attachments` lists staged files
- `/detach <index|path>` removes a staged file
- next submitted prompt sends the staged files with the message
- staged files clear after a successful send or when `/new` resets the conversation

### Architecture fit

- Keep the existing `main.rs` event loop and `pending_submit` send path.
- Add attachment state to `App`, not the input buffer.
- Extend `DustClient` with upload and content-fragment helpers, then add a send flow that accepts staged attachments.
- Reuse the existing chat mode and avoid adding a modal for v1.

### File-by-file changes

#### `src/app.rs`

- Add a staged attachment model, likely storing:
  - original path
  - display name
  - size
  - guessed content type or a placeholder
- Add methods:
  - `stage_attachment(...)`
  - `remove_attachment(...)`
  - `clear_attachments()`
  - `attachments()`
- Clear attachments in `new_conversation()`
- Decide whether attachments persist across `/resume`; recommended: keep them until used or explicitly cleared

#### `src/handler.rs`

- Extend `SlashCommand` to support parameterized commands:
  - `Attach(String)`
  - `Detach(String)`
  - `Attachments`
- Update `parse_slash_command` to parse command arguments rather than exact string equality only
- Add unit tests for argument parsing and whitespace handling

#### `src/slash.rs`

- Register new built-ins:
  - `/attach`
  - `/attachments`
  - `/detach`
- Add descriptions tuned for terminal workflows

#### `src/dust/types.rs`

- Add serde types for:
  - file upload request/response
  - uploaded file metadata
  - content fragment request/response
- Keep them minimal and only cover fields Oxide needs for upload plus attachment association

#### `src/dust/client.rs`

- Add:
  - `upload_file(...)`
  - `post_content_fragment(...)`
  - `send_message_flow_with_skills_and_attachments(...)`
- For a new conversation:
  - upload files first
  - pass `contentFragments` into conversation creation
- For an existing conversation:
  - upload files first
  - create content fragments with `POST /content_fragments`
  - post the user message after fragments exist
- Preserve the existing streaming logic and error reporting
- Keep upload implementation close to Dust SDK behavior:
  - request upload URL
  - POST multipart form data to the returned `uploadUrl`
  - then associate returned `fileId`

#### `src/main.rs`

- On `/attach`, validate the path and stage it in `App`
- On `/attachments`, emit a system message summarizing staged files
- On `/detach`, remove the requested staged file and confirm via system message
- When `pending_submit` is drained, call the new client send flow with both active skills and staged attachments
- After successful send setup, clear staged attachments in app state
- If upload/send fails, keep staged attachments so the user can retry

#### `src/ui/messages.rs`

- Render staged attachments in chat view, preferably as a compact system-style block near the bottom when any are pending
- Render sent attachments as part of the user message only if that can be done without rewriting the message model too much; otherwise defer to a later phase

### Dust API notes

- Dust SDK evidence shows the intended flow is:
  - upload file
  - convert uploaded file ids into content fragments
  - attach fragments to conversation creation or append them to existing conversation before posting the next message
- The swagger file documents `POST /api/v1/w/{wId}/files` and `POST /api/v1/w/{wId}/assistant/conversations/{cId}/content_fragments`
- Retry endpoint versioning is inconsistent between swagger sections and SDK references; attachments do not depend on that ambiguity, which is another reason they are the best first slice

### Testing strategy

- Unit tests:
  - slash command parsing for attachment commands
  - app staged attachment state transitions
  - request serialization for file upload and content fragments
  - URL builders for new Dust client methods
- Integration-style unit tests in `src/dust/client.rs`:
  - multipart upload request setup if practical
  - correct branching between new conversation and existing conversation flows
- Manual tmux test:
  - start `cargo run`
  - `/attach` a small text file
  - verify staged attachment rendering
  - send a prompt asking the agent to summarize the file
  - verify upload succeeds and staged list clears

### Milestones

1. Add attachment slash commands and app state.
2. Add Dust file/content-fragment client methods and serde types.
3. Wire attachment-aware send flow into `main.rs`.
4. Add pending-attachment rendering in the chat UI.
5. Add tests and do a tmux pass.

## Follow-on Sequence After Attachments

Recommended next order:

1. Action timeline + blocked-action retry
2. Structured question answering
3. In-conversation agent mentions
4. Toolset toggling
5. Artifact download

## Unknowns / Decisions

- Whether `/attach` should accept globs in v1 or just exact paths
- Whether attachments should be uploaded immediately on `/attach` or deferred until send
  - recommendation: defer until send to avoid orphan uploads
- Whether sent attachments must remain visibly attached to the historical user message in v1
  - recommendation: not required for first slice
- Whether to allow directories or only regular files
  - recommendation: regular files only in v1
