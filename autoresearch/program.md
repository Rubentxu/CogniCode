# CogniCode Self-Evolving Rules — Program v2

## Goal
Improve CogniCode's 862 code quality rules through autonomous experimentation.
Improvements can be:
1. **Metric improvements**: Better precision, recall, F1, lower false-positive rate
2. **Code quality improvements**: Better SOLID compliance, Clean Code, segregation
3. **Structural improvements**: Extracting rules from monolithic catalog.rs to individual files

## Architecture
You are part of a self-evolving rule system. Your roles:

- **Analyzer**: Read metrics + code quality scores, identify rules needing improvement
- **Improver**: Read rule code, propose improvements, edit files, segregate if beneficial
- **Evaluator**: Run test suite + sandbox + external tools on modified code
- **Decider**: Compare baseline vs current, decide keep/discard

## SOLID Principles as Improvement Criteria

### SRP — Single Responsibility Principle
**One rule = one file = one responsibility.**

- ✅ KEEP: Rule extracted from catalog.rs into its own file
- ✅ KEEP: Rule file only contains ONE declare_rule! block + its tests
- ❌ DISCARD: File with multiple unrelated rules

### OCP — Open/Closed Principle
**Rules should be extensible without modification.**

- ✅ KEEP: Parameters/thresholds extracted as configurable fields
- ✅ KEEP: Pattern list is extendable (vec of patterns, not hardcoded if-else)
- ❌ DISCARD: Hardcoded magic numbers that require code changes to tune

### LSP — Liskov Substitution Principle
**All rules implement the Rule trait consistently.**

- ✅ KEEP: Rule correctly implements all trait methods
- ✅ KEEP: explanation(), clean_code_attribute(), software_qualities() all return valid data
- ❌ DISCARD: Rule that panics or returns invalid data for any trait method

### ISP — Interface Segregation Principle
**Rules only import what they need.**

- ✅ KEEP: Minimal imports (only Severity, Category, Issue, Rule, RuleContext)
- ❌ DISCARD: Unused imports, wildcard imports (use crate::*)

### DIP — Dependency Inversion Principle
**Rules depend on abstractions (Rule trait), not concretions.**

- ✅ KEEP: Rule uses RuleContext trait methods, not direct tree-sitter calls
- ❌ DISCARD: Rule bypasses the trait to access internal implementation details

## Clean Code Criteria

| Criterion | KEEP | DISCARD |
|-----------|------|---------|
| **Naming** | Rule struct: `S134Rule`, file: `s134_deep_nesting.rs` | Abbreviated, unclear names |
| **Function size** | check() body < 50 lines | check() body > 100 lines |
| **Imports** | Only necessary imports, no unused | Unused imports, `*` imports |
| **Documentation** | File-level `//!` doc, explanation field populated | No documentation |
| **Tests inline** | `#[cfg(test)] mod tests` in same file | No tests |
| **Regex readability** | Named capture groups, comments with `(?x)` | Opaque regex without comments |

## Rules (NEVER violate these)

1. NEVER modify the evaluation harness (corpus, baseline/, metrics.db, consensus engine)
2. NEVER modify types.rs (the Rule trait interface)
3. NEVER delete existing tests — only add or improve
4. ALWAYS verify compilation: `cargo check -p cognicode-axiom`
5. ALWAYS run full test suite: `cargo test --workspace`
6. NEVER install new dependencies without human approval
7. NEVER skip the consensus evaluation step

## What you CAN modify

### Rule files (individual or catalog.rs):
- Regex patterns in the check closure
- Detection thresholds and parameters
- Logic improvements in the check closure body
- Metadata (explanation, clean_code, impacts)

### File organization:
- ✅ MOVE a rule from catalog.rs to `rules/{lang}/{category}/{id}_{name}.rs`
- ✅ CREATE new rule file following the naming convention
- ✅ UPDATE mod.rs files to register new/existing rules
- ✅ RUN `generate_registry.py` after file changes

### File structure convention:
```
rules/{language}/{category}/{rule_id}_{short_name}.rs

Examples:
  rules/rust/security/s2068_hardcoded_credentials.rs
  rules/rust/code_smells/s134_deep_nesting.rs
  rules/python/security/py_s1523_eval.rs
  rules/js/security/js_s2611_innerhtml_xss.rs
```

## Decision Criteria

**KEEP if ANY of these hold:**

### Metric Improvements
- ΔF1 > 0.01 AND ΔFPR < 0.05 → real improvement
- Rule was broken (F1=0) and now works (F1>0) → recovery

### Code Quality Improvements (no metric regression required)
- Rule extracted from catalog.rs → individual file (SRP) ✅ ALWAYS KEEP
- Code simplified: ≥20% fewer lines, same functionality ✅ KEEP
- SOLID compliance improved (any principle, no regression elsewhere)
- Clean Code: better naming, documentation, test coverage
- Unused imports removed, magic numbers extracted to params

### Simplicity Gains
- Same metrics, less code → KEEP
- Same metrics, better organized → KEEP
- Removed dead code or redundant patterns → KEEP

**DISCARD if ANY:**
- F1 decreases by > 0.02 (significant regression)
- FPR increases by > 0.05 (FP explosion)
- Tests fail
- Compilation fails after 3 repair attempts
- Execution time increases > 20% without metric improvement
- Code becomes MORE complex without metric gain

## File Segregation Protocol

When extracting a rule from catalog.rs:

1. Create new file: `rules/{lang}/{category}/{id}_{name}.rs`
2. Copy the full `declare_rule!` block + tests
3. Add file-level doc comment (`//! Rule description`)
4. Add to parent `mod.rs`: `pub mod {filename};`
5. Run `generate_registry.py` to update registry.rs
6. Remove from catalog.rs (replace with re-export comment)
7. Run `cargo check` + `cargo test`
8. If all pass → KEEP, else → REVERT

## Output Format

Log EVERY experiment to `autoresearch/evolution.tsv`:

```
iteration  rule_id  f1_before  f1_after  fpr_before  fpr_after  solid_score  decision  change_type  description
1          S134     0.78       0.82      0.03        0.03       2            keep      metric       tighten nesting detection
2          S2068    0.89       0.89      0.01        0.03       4            keep      segregation  extracted to s2068_hardcoded_credentials.rs
3          S3776    0.65       0.65      0.08        0.08       3            keep      solid        removed magic numbers, extracted thresholds
```

**change_type values**: `metric` | `segregation` | `solid` | `clean_code` | `simplify`

## SOLID Score

Each rule receives a SOLID score (0-5, one point per principle):

```
Score = SRP(0|1) + OCP(0|1) + LSP(0|1) + ISP(0|1) + DIP(0|1)

SRP: 1 if rule is in its own file (or catalog.rs is the only rule in scope)
OCP: 1 if thresholds/patterns are configurable via params
LSP: 1 if all trait methods return valid data (no panics, no None where expected)
ISP: 1 if imports are minimal (≤5 specific imports)
DIP: 1 if rule uses RuleContext trait, not raw tree-sitter
```

## NEVER STOP

Once started, do NOT pause to ask for permission. Continue until 
max_iterations reached or manual interrupt.

If you run out of improvement ideas:
1. Check for rules still in catalog.rs → segregate them
2. Check SOLID scores → improve lowest-scoring dimension
3. Check Clean Code criteria → improve naming, docs, test coverage
4. Re-read rule patterns — look for edge cases
5. Check cross-language variants — unify approach
6. Review the rule's own explanation for improvement hints
