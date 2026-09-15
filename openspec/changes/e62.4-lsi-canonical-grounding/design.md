# Design — cycle e62.4 — canonical grounding, coherence verification (U42, part 2b)

> Cycle: A-lite | Milestone: M6 | Phase: design | Date: 2026-09-15

## D1 — Why the binding list is index-aligned and not a map

`DetectorMatch::evidence` and `CausalObservation::evidence` are indices into
`produced_evidence`. Any representation that removes entries renumbers them, and
a silently repointed match is exactly the class of bug this milestone exists to
prevent. `EvidenceBindings` therefore has the same length as the produced
evidence and answers by index.

```text
produced[0] → Grounded { id: 4, fact: 7 }
produced[1] → Ungrounded { NoFact }
produced[2] → Grounded { id: 5, fact: 9 }
```

## D2 — The fact of a causal step is derived, never declared

`CausalObservation` used to carry a `fact` field that the assembler copied into
`CausalStep.fact`. Two sources of truth for one claim means one of them is
unverified, so the field is gone: the assembler sets `CausalStep.fact` from the
*evidence binding* of the atom the step cites, or leaves both `evidence` and
`fact` unset when the atom is ungrounded.

```text
CausalStep.fact = bindings[step.evidence].fact
```

There is now no way for a backend to state a fact its evidence does not grade.

## D3 — Grade, class and grounding are three different questions

```text
EvidenceClass  strength of the analysis     (findings, A–D)
Grounding      connection to canonical truth (Some/None + FactId)
EvidenceGrade  relation of evidence to fact  (kernel: Supports/Refutes/Corroborates)
```

A dataflow route can be class B, valid, well-explained, and still ungrounded.
Any one of the three being wrong is enough to refuse the gate, and none of them
is derivable from the others, so they are checked independently. In particular
`EvidenceClass` stays out of the kernel read model — it is not a grade.

## D4 — Ungrounded is a value, not an error

The bridge is a *translator*, not a gate. It reports what it could and could not
ground and lets the verifier refuse. The one thing it never does is degrade a
store failure into "ungrounded": a store that cannot answer is an error, because
"we could not check" and "there was nothing to find" must not be the same
outcome.

## D5 — Where the I/O lives

```text
domain       DetectorExecutor::prepare  → PreparedExecution   (no I/O)
application  CanonicalEvidenceWriter.persist                  (async, kernel)
domain       PreparedExecution::finalize(bindings) → Finding
application  KernelEvidenceReadModel::load                    (async, kernel)
domain       FindingVerifier::verify_for_gate                 (sync, pure)
```

`PreparedExecution` has private fields and one constructor, so the async
detour cannot bypass admission, planning or the backend-contract checks. The
read model does its I/O once and then answers synchronously, which is why the
verifier never becomes async.

## D6 — `EvidenceGrade` moves to ungated vocabulary

The verifier must be able to say "this evidence refutes its fact" without the
whole kernel compiled in. `EvidenceGrade` is fundamental domain vocabulary, not
infrastructure, so it moves to `domain::kernel_ids` exactly as the ids did in
e56, and `evidence_kernel::evidence` re-exports it. Duplicating the enum in the
findings module was the alternative, and it is the one thing that must not
happen: two grades would eventually disagree.

## D7 — Ambiguity fails closed

If a hop between two nodes is witnessed by parallel edges naming *different*
facts, no fact can be said to ground the hop. The hop's atom is left ungrounded
and a diagnostic names the relation; the witness itself is still reported,
because the reachability result is real. Choosing one edge would be inventing a
causal claim, and dropping the finding would throw away a true observation.
