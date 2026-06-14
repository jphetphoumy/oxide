# ADR-0009: Local skills as a built-in MCP tool

**Date**: 2026-06-14
**Status**: Accepted

## Context

Oxide needs a way to load reusable instruction sets ("skills") from local markdown files and make them available to Dust agents. A skill is a `.md` file with YAML frontmatter (`name`, `description`) and a body of instructions. Skills live in `.agents/skills/` relative to the current working directory, following the [agentskills.io](https://agentskills.io) spec.

Three injection mechanisms were investigated:

**`contentFragments`** — tested: Dust acknowledges the document but surfaces its own native skills list, not the injected content. Ruled out.

**`MessageContext` extension** — no `instructions` or `system_prompt` field exists in the Dust API. Not available.

**Dust memory / knowledge system** — requires a data source connected to the agent configuration; not wirable from the CLI at request time. Not viable.

**First-message content prepend (full instructions)** — works, but duplicates skill content into conversation history on every new conversation. Wasteful; the agent re-reads what oxide already serialized.

## Decision

Skills are exposed as a **built-in MCP tool: `oxide_skill`**. The agent calls `oxide_skill(skill_id)` to load a skill's full instructions on demand. Oxide reads `.agents/skills/<skill_id>.md` and returns the file content.

This follows the same pattern as the existing `oxide_bash` built-in — registered in `McpManager::init()`, dispatched in `McpManager::call_tool()`.

### Auto-registration

`oxide_skill` is registered **automatically** when `McpManager` initialises, without requiring a `[[mcp.servers]]` config entry. Skills are read-only file reads — no security concern warrants explicit opt-in. If `.agents/skills/` does not exist, the tool is still registered (it returns a clear error on call rather than silently disappearing from the tool list).

### Tool schema

```json
{
  "name": "oxide_skill",
  "description": "Load the full instructions for a local skill from .agents/skills/",
  "inputSchema": {
    "type": "object",
    "properties": {
      "skill_id": {
        "type": "string",
        "description": "Filename stem of the skill, e.g. \"code-review\" for .agents/skills/code-review.md"
      }
    },
    "required": ["skill_id"]
  }
}
```

### First-message index

When one or more skills are active (set via `/skills:<id>`), a compact index is prepended to the first user message at conversation creation. It lists available skills and instructs the agent to call `oxide_skill` to load them:

```
# Oxide local skills
code-review: Help review code for correctness, simplicity, and security
sql-expert: Optimize SQL queries and schema design

Use oxide_skill(skill_id) to load a skill's full instructions.

{user's first message}
```

Only name and description are injected — not the instructions body. The agent requests the content explicitly via tool call.

### Activation model

- `/skills:<id>` appends to `active_skills: Vec<Skill>` on `App`, cleared on `/new`
- Multiple skills can be active simultaneously
- Activation is per conversation — `/resume` does not restore previous skills
- Status bar shows all active skill names when any are set

### Skills as slash commands

Skills discovered from `.agents/skills/` at startup register as `/skills:<id>` slash commands (e.g. `code-review.md` → `/skills:code-review`). The existing static `COMMANDS` array in `slash.rs` is extended at runtime. A single `SlashCommand::ActivateSkill(String)` variant handles all skill commands.

## Consequences

- **Zero Dust API changes** — injection is pure client-side string formatting.
- **Agent loads skill on demand** — instructions body never leaves the local disk unless the agent explicitly calls `oxide_skill`. Conversation history stays clean.
- **Works without file system MCP** — the agent does not need a bash or file tool; `oxide_skill` is always available as a first-class oxide built-in.
- **Multiple skills** — `active_skills: Vec<Skill>` replaces `active_skill: Option<Skill>`. All active skills appear in the first-message index.
- **Auto-registered** — no config entry needed. `oxide_skill` appears in the agent's tool list automatically, like a native capability.
- **`.agents/skills/` is canonical** — follows the agentskills.io spec, not configurable.
- **Dynamic slash commands** — `slash.rs` gains a runtime-appended skill section. The `SlashCommand::ActivateSkill(String)` variant is matched by prefix pattern rather than individual variants.
