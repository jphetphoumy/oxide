# ADR-0006: Match Dust CLI request identity

**Date**: 2026-06-01
**Status**: Accepted

## Context

Oxide uses the same WorkOS login flow and Dust API surface as the official Dust CLI. The implementation plan in `docs/plan/dust-api.html` explicitly calls out that the official client sends:

- `User-Agent: Dust CLI`
- `origin: "cli"` in message context

These values are used by Dust to distinguish CLI traffic from web traffic and to keep usage tracking aligned with the official client behavior.

During implementation, Oxide briefly used its own user agent string (`oxide/<version>`). That diverged from the documented plan and from the behavior we want to emulate.

## Decision

Use the same request identity markers as the official client:

```text
User-Agent: Dust CLI
origin: "cli"
```

This applies to the shared HTTP client used for authentication today and should remain the default for Dust API calls and message context added later.

## Consequences

- Oxide matches the documented integration plan and the official Dust CLI behavior
- Dust sees Oxide traffic as CLI traffic in the same way as the official client
- We give up a custom Oxide-specific request identity for now
- If Oxide needs its own product identity later, we should revisit this explicitly rather than drifting from the plan ad hoc
