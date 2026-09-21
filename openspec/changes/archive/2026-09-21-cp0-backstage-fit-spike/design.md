# CP0 — Design

## Integration shape (service-backed, mandated)

```text
Backstage frontend plugin      packages/plugin-cognicode
        │  GET /api/cognicode/probe   (same-origin, host-authenticated)
        ▼
Backstage backend plugin       packages/plugin-cognicode-backend
        │  GET {cognicode.baseUrl}/control-plane/probe
        ▼
CogniCode Rust service (Explorer API)   ← source of truth
```

## Allocation of responsibility

| Layer | Owns | Does NOT own |
|---|---|---|
| Backstage frontend | rendering, navigation, handoff link | domain logic |
| Backstage backend | config, HTTP proxy, actor→identity context | canonical state, domain logic |
| CogniCode Rust | truth: findings, evidence, approvals, permits | UI |

## ContextCapsule v1 (WU6)

The handoff contract between host and Explorer. Invariants:

- **versioned** (`CONTEXT_CAPSULE_VERSION = 1`); unknown versions are rejected, never guessed;
- **bounded** (`MAX_FOCUS_REFS = 32`, `MAX_REF_LENGTH = 512`);
- **references only** — identifiers, never copied Facts/Evidence;
- **no authority** — it cannot express approval, promotion or authorship;
- **deterministic** — focus is deduped and sorted, so equal inputs are byte-identical.

Handoff is a **link**, not an iframe. The Explorer owns the inner loop.

## Identity crossing the boundary is CONTEXT, not authority

The host's authenticated actor is forwarded as `x-cognicode-actor-subject` /
`x-cognicode-actor-source`. The probe response reports `"authority": "none"`.
This mirrors the existing LSI invariant: `ActorRef::Human` is caller-constructible
data, never proof. A header cannot promote a world.
