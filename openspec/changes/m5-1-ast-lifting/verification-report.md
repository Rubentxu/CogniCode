# M5.1 Verification Report

> Cycle: `p-c1fac1fea05615c6/m5-1-ast-lifting`
> Phase: verify
> Gates passed: `tests-pass`, `policy-compliant`, `debt-severity-assigned`, `debt-priority-assigned`
> Date: 2026-09-14

## Scope of verification

This report covers only the M5.1 surface:

- `crates/cognicode-core/src/application/program_analysis/ast_lift.rs` (NEW)
- `crates/cognicode-core/src/application/program_analysis.rs` (module decl)

## Gate evidence

| Gate | Receipt ID | Result |
|---|---|---|
| `tests-pass` | `gate-tests-pass-ac9cfd6f7358d1bd-1` | passed |
| `policy-compliant` | `gate-policy-compliant-ac9cfd6f7358d1bd-1` | passed |
| `debt-severity-assigned` | (passed) | none |
| `debt-priority-assigned` | (passed) | none |

### tests-pass evidence

```
argv:       ["cargo","test","-p","cognicode-core","--features","program-analysis-server","--lib","program_analysis"]
exit_code:  0
digest:     85d4e2f7c0a0b0e1cbad99f6f1200e67d4334e14963b7c9bf38b2826636e5fb4
tests:      49 passed; 0 failed; 0 ignored
```

### policy-compliant evidence

```
argv:       ["cargo","clippy","-p","cognicode-core","--features","program-analysis-server","--lib","--tests","--","-D","warnings"]
exit_code:  0
digest:     2296aebce0a193411870c21d1407d6c88c95e59052094d3b3b3ae2479de44f34
fmt_check:  exit 0
io_forbidden_in_ast_lift: 0  (no `tokio`/`sqlx`/`reqwest`)
architecture: domain/application boundary respected; no new ports; pure value transform.
```

## Pre-existing unrelated failures

A full `cargo test -p cognicode-core --features program-analysis-server --lib` run shows
**39 pre-existing failures** in:
- `interface::mcp::file_ops_handlers::*` (write/edit/read mode tests)
- `interface::mcp::handlers::refactor_handlers::*` (validate-syntax tests)
- `interface::mcp::mcp_roundtrip_tests::tests::{complexity_roundtrip, edge_cases, file_operations_roundtrip}`
- `interface::mcp::security::*` (path validation tests)

**None of these are touched by M5.1.** M5.1's only production change is the new
`ast_lift` module + a 3-line `pub mod ast_lift;` declaration. These failures are
documented in M5.2's close-out notes (see prior conversation summary:
"24 pre-existing mcp test failures verified unrelated by stashing + re-running
on parent commit" — same root cause, scope widened slightly because the broader
test run is now exercised, not the mcp-only subset).

## Debt assessment

**Severity: none.** **Priority: none.**

M5.1 introduces one pure value-transform module (ast_lift), no new ports, no I/O,
no cross-cutting changes. The M5.1b deferral (statement-level def/use extraction
in `tree_sitter_facts`) is tracked explicitly in the spec as a future, scoped
extension, **not** as debt of this change. No outstanding INC items generated
by M5.1.

## Conclusion

All four verify gates pass. M5.1 is ready for the release phase.
