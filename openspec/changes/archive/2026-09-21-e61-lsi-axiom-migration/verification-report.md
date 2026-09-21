# Verification Report — cycle e61 — selective Axiom → DetectorIr migration

> Cycle: A-lite | Milestone: M6 | Phase: verify | Date: 2026-09-15

## Summary

| Item | Value |
|------|-------|
| Cycle | e61 — selective Axiom → DetectorIr migration (umbrella 7.7) |
| Path | A-lite |
| Base HEAD | `a3d26360` (post-e60.1) |
| Verify verdict | **PASS** |

## Verification

### V-1 — Tests

```
cargo test -p cognicode-core --lib application::findings            # 21 passed
cargo test -p cognicode-core --test findings_axiom_import_e2e      # 3 passed
cargo test -p cognicode-core --lib domain::findings                # 98 passed
cargo test -p cognicode-core --test findings_ast_e2e               # 6 passed
cargo test -p cognicode-core --test findings_graph_e2e             # 4 passed
cargo test -p cognicode-core --test findings_dataflow_e2e          # 7 passed
```

### V-2 — The required UATs

| UAT | Test |
|-----|------|
| U-A1 supported AST rule → valid IR | `u_a1_ast_rule_translates_to_a_valid_detector_ir` |
| U-A2 legacy BLOCKER stays `Candidate` | `u_a2_legacy_blocker_maps_to_policy_not_authority` |
| U-A3 metadata-only → skipped, no invented IR | `u_a3_metadata_only_rule_is_skipped` |
| U-A4 capability derived from semantics | `u_a4_capability_is_derived_from_semantics` |
| U-A5 incompatible → loud stable diagnostic | `u_a5_unsupported_analysis_is_skipped_loudly` |
| U-A6 same rule/revision ⇒ same semantic digest | `u_a6_stable_translation` |
| U-A7 severity change ⇒ logic equal, policy+semantic move | `u_a7_severity_change_moves_policy_not_logic` |
| U-A8 imported detector uses the same chain | `u_a8_imported_detector_uses_the_same_chain_as_a_builtin` (+ promotion via a governance verifier) |

### V-3 — No fabricated detectors

`LegacyDetection::None` and `LegacyDetection::Unsupported` return
`Skipped` with `MetadataOnly` / `UnsupportedAnalysis`; a non-namespaced
subject returns `Unmappable`. No path emits an approximate `DetectorIr`.

### V-4 — Build / format / regressions

- `cargo check --workspace --all-targets` → 0 errors.
- `cargo fmt --all --check` → clean.
- known-failure baseline unchanged (41).

## Conclusion

Legacy rules migrate **selectively**: what can be expressed faithfully becomes a
validated `DetectorIr`; what cannot is skipped with a stable diagnostic. No
legacy severity or quality gate ever yields execution authority. PASS.
