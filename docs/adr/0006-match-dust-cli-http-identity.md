# ADR-0006: Match Dust CLI request identity

**Date**: 2026-06-01
**Status**: Accepted

## Context

Oxide is a Rust Dust CLI, not a separate product identity. It uses the same WorkOS login flow and Dust API surface as the official Dust CLI, and the implementation plan in `docs/plan/dust-api.html` explicitly calls out that the official client sends:

- `User-Agent: Dust CLI`
- `X-Dust-CLI-Version: 0.4.5`
- `origin: "cli"` in message context

These values are used by Dust to distinguish CLI traffic from web traffic and to keep usage tracking aligned with the official client behavior. Oxide should remain aligned with the official Dust CLI version signature rather than inventing its own version header.

During implementation, Oxide briefly used its own user agent string (`oxide/<version>`). That diverged from the documented plan and from the behavior we want to emulate.

## Decision

Use the same request identity markers and version signature as the official client:

```text
User-Agent: Dust CLI
X-Dust-CLI-Version: 0.4.5
origin: "cli"
```

This applies to the shared HTTP client used for authentication today and should remain the default for Dust API calls and message context added later. Oxide may be better ergonomically, but it should still present itself as the official Dust CLI on the wire.

## Consequences

- Oxide matches the documented integration plan and the official Dust CLI behavior
- Dust sees Oxide traffic as CLI traffic in the same way as the official client
- We give up a custom Oxide-specific request identity for now, by design
- If Oxide ever needs its own product identity later, we should revisit this explicitly rather than drifting from the plan ad hoc
