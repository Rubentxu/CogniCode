# Proposal — e77 LSI Executable Architecture

> Cycle: A-lite | Milestone: M10 — Executable Architecture (e77 first slice) | Phase: propose | Date: 2026-09-17

## Status

```text
M9 implementation        CLOSED  (e71 + e72 + e73; reconciled 2026-09-17)
M9 strategic decision    PASSED  (user directive 2026-09-17)
e74 / e75 / e76          CLOSED (M14 — portable runtime / execution / self-hosting)
e77                      PROPOSE  (this document)
ADR-049                  PROPOSED (remains PROPOSED until its own acceptance /
                                 evidence requirements are satisfied — the same
                                 separation rule that applies to ADR-046 applies here)
```

## Goal

Convert architectural intent (an ADR or any human-curated source) into **machine-executable
constraints** that evaluate against the canonical fact/graph surface and emit grounded
`ArchitectureDriftFinding` instances — without ever giving an ADR text automatic execution
or gating authority.

If this cycle lands, CogniCode becomes a system that knows, in a verifiable way, **whether
a real codebase respects a stated architecture rule**, and can gate CI on that knowledge
through the existing M8/M9 evidence + gate stack.

The first concrete consumer is **CogniCode over CogniCode**: e76's self-hosting mutation
corpus gains architectural mutations (e.g. `domain/foo.rs → use crate::infrastructure::…`)
with pre-declared outcomes, and e77 verifies that the resulting drift findings are correct.

## The chain (mandatory)

```text
ADR / architecture intent
        ↓
ConstraintCandidate            (human-curated; machine-readable proposal)
        ↓
approval / admission            (explicit, audit-logged; creates ArchitectureConstraint)
        ↓
ArchitectureConstraint          (the only surface with evaluation authority)
        ↓
analysis                        (against canonical facts + graphs)
        ↓
graph / fact evidence           (concrete: edge X exists, node Y has label Z)
        ↓
ArchitectureDriftFinding        (grounded: the fact that violates the constraint)
        ↓
EvidenceBundle                  (M8 surface: bundles drift findings + supporting evidence)
        ↓
PolicyGate                      (M8 surface: verdict; the gate is the gate)
```

The chain reuses M6 (findings + canonical evidence), M8 (EvidenceBundle + PolicyGate) and
M9 (TrialExecutor + promotion). e77 introduces **no new authority path**; it composes the
existing authorities.

## The fundamental rule

```text
ADR text alone has ZERO execution / gating authority.
```

A constraint candidate must be:

1. **Machine-readable** (not free-form text — a typed `ArchitectureRule`).
2. **Explicitly admitted** by an approved authority (human curator for now;
   M11's `AutomatedAuthorPromotionPolicy` may add an automated curator later).
3. **Versioned** (constraints change; old versions must remain auditable).

An ADR or markdown file may **suggest** a candidate. It does not, by itself, become a
candidate; it does not, by itself, become a constraint; it does not, by itself, gate
anything.

This is the same `creation != authority` rule that e72 WU1 established for `ChangeProposal`
and e73 WU2 established for `PromotionPermit` — applied now to the architecture layer.

## Minimal `ArchitectureRule` taxonomy (3 rules, not 20)

Per your directive: **start with 2–3 real rules**, not a generic framework.

```rust
ArchitectureConstraintId   // opaque

ArchitectureConstraint {
    id,
    version,            // bumps on rule evolution; old versions stay auditable
    source,             // ADR id + curator + admission event id
    rule: ArchitectureRule,
    scope: ArchitectureScope,   // which workspace(s) + snapshot(s)
    authority: ConstraintAuthority,  // who admitted it
}

enum ArchitectureRule {
    LayerDependency { from: Layer, may_depend_on: Vec<Layer> },
    ForbiddenDependency { from: Layer, to: Layer },
    NamespaceBoundary { inside: Namespace, may_not_import: Vec<Namespace> },
}

enum Layer {
    Domain,
    Application,
    Infrastructure,
    Presentation,  // MCP / explorer / IDE adapter
}
```

Three concrete rules to start with (CogniCode over CogniCode):

| Rule | Verbatim constraint |
|------|---------------------|
| R1 | `domain` MUST NOT depend on `infrastructure` |
| R2 | `domain::evidence_kernel` MUST NOT depend on `presentation` or MCP |
| R3 | `application` MAY depend on `domain::ports` but NOT on `infrastructure` concretes |

These three rules already encode a hexagonal-architecture invariant (the existing
architecture rule documented in AGENTS.md: *"domain code MUST NOT import sqlx, tokio, or
any I/O crate"*). The cycle proves they are machine-checkable.

## Reuse the e76 self-hosting consumer

The e76 mutation corpus already has 7+ pre-declared mutations (add/remove/rename function,
line reorder, BOM/CR/LF, partial divergence). e77 extends the corpus with **architectural
mutations**:

```text
domain/foo.rs
   ↓
use crate::infrastructure::...
   ↓
expected:
  ArchitectureBoundaryViolation
   ↓
CogniCode analyzes CogniCode
   ↓
finds the forbidden edge
   ↓
ArchitectureDriftFinding (grounded)
   ↓
evidence: the edge + the constraint + the ADR
```

This is the **vertical** e77 closes:

```text
architectural intent (ADR)
        ↓
machine-readable constraint (ArchitectureConstraint)
        ↓
real code graph (canonical facts)
        ↓
evidence (the violating edge)
        ↓
finding (ArchitectureDriftFinding)
        ↓
self-host validation (e76 corpus proves CogniCode got it right)
```

## Proposed work units

```text
WU0  Architecture ownership map               (no code; documents which layer
                                              owns admission + evaluation)
WU1  ArchitectureConstraint model            (types; ArchitectureRule enum;
                                              scope; authority)
WU2  Candidate → approved admission          (the explicit gate; admission event
                                              is audit-logged; produces an
                                              ArchitectureConstraint)
WU3  Architecture evaluator                   (consumes ArchitectureConstraint +
                                              canonical facts/graphs; emits a
                                              stream of graph observations)
WU4  Drift Finding + canonical evidence      (ArchitectureDriftFinding is a
                                              subtype of Finding, grounded in
                                              M6 evidence; supports M8 bundle)
WU5  CogniCode self-host architectural       (extends e76's mutation corpus with
                                              architectural mutations; pre-declared
                                              outcomes)
WU6  UAT + adversarial                       (acceptance: the vertical works;
                                              adversarial: ADR text alone is
                                              zero authority; contradictions;
                                              missing inputs; etc.)
```

## Adversarial matrix (essential)

| Case | Expected outcome |
|------|------------------|
| Candidate constraint (not yet admitted) → against graph | **No** finding (no authority yet) |
| Approved constraint + no violation in graph | **No** finding |
| Approved constraint + violating edge in graph | **Grounded** `ArchitectureDriftFinding` |
| `ArchitectureDriftFinding` without graph evidence | **Refused at the gate** (cannot ground) |
| Constraint referencing missing / unknown ADR | `InvalidConstraint` (incomplete) — fail-closed |
| Two contradictory approved constraints | Conflict surfaced; **no silent resolution** |
| ADR text alone (no admission event) | **ZERO** findings emitted, regardless of what the ADR says |
| Constraint admitted by a non-approved curator | Refused at admission; no constraint produced |

The seventh case is the load-bearing one. It encodes the rule:

```text
ADR text alone
   → ZERO authority
```

## Out of scope (deferred, per your directive)

- **Generalized Packs (e78)**: BLOCKED until at least 2 real pack consumers exist. The
  two candidates are:
  1. Detector packs (M8 era)
  2. Architecture constraint packs (this cycle)
  If both have concrete, overlapping needs at the end of e77, e78 may be authorized;
  otherwise deferred.
- **M11 AI Investigation / Critic / Fix Agent**: BLOCKED until
  `AutomatedAuthorPromotionPolicy` lands. The e73 permit/promotion surface is the
  substrate; the policy is the seam.
- **ADR parsing via LLM**: explicitly out of scope. Initially all candidates are
  human-curated. M11 may add automated proposal later, with the policy seam in place.
- **macOS / Windows UAT, query-level dependencies, distributed workers, CloudEvents**:
  all out of scope; no new feature until e77 lands.

## Spec delta

None. e77 introduces no new umbrella requirements; it refines M10 (Executable Architecture)
at the application-layer seam. Per AGENTS.md, requirement ids stay in the umbrella change;
the cycle's archive-manifest will carry the per-WU commitments.

## Acceptance gate (for the verify phase)

| Gate | Outcome |
|------|---------|
| `cargo test -p cognicode-core --features evidence-kernel architecture` | GREEN |
| Self-host vertical: CogniCode analyzes CogniCode and finds the forbidden `domain → infrastructure` edge | GREEN (e76 corpus extension) |
| ADR text alone emits **zero** findings | GREEN |
| Two contradictory constraints surface a conflict | GREEN |
| Missing ADR is refused | GREEN |
| `cargo fmt --check -p cognicode-core` | clean |
| `cargo clippy -p cognicode-core --all-targets --features evidence-kernel -- -D warnings` | clean on touched paths |

## Lifecycle note (honest)

This is a **proposal**, not yet a closed cycle. The cycle will be opened when this
proposal is approved and the WUs start. Following the e68-e70+e74-e76+e71-e73 pattern, the
likely path is A-lite (degraded-but-governed if worker delegation remains unavailable per
DEBT-SDDK-003), D3-DEFER for the formal lifecycle per DEBT-SDDK-002. No formal SDDK ledger
row is created until a cycle is opened via `sddk cycle start`.

Architecture intent and approval are separated by design. ADR-049 stays PROPOSED; e77 is
the *spike* whose outcome (if GREEN) is the evidence that ADR-049 needs to become ACCEPTED.
