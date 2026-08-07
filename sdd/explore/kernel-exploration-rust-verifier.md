# Kernel Exploration: C10 — Rust Verifier Service

## Current State

The rust verification capability **already exists** but is embedded as a concrete method on
`FileOperationsService` without a domain port/trait abstraction.

### What exists today

**`FileOperationsService::retrieve_and_verify()`** (`file_operations.rs:1644`) is a fully working
implementation that:

1. **Searches** for code matching a query via the existing `search_content` infrastructure
2. **Verifies** each `.rs` candidate via `rustc --edition 2021 --crate-type lib` in a sandboxed
   temp directory (`cognicode_rust_verify_` prefix)
3. **Times out** per file at 10s with `tokio::process::Command` + `kill_on_drop(true)`
4. **Classifies** results as `Verified`, `Rejected`, or `Skipped`

Three verification methods exist:
- `verify_rust_file()` (sync, line 1500) — sync compilation via `std::process::Command`
- `verify_rust_file_with_timeout()` (async, line 1571) — tokio async with timeout
- `run_rustc_async()` (async helper, line 1482) — tokio subprocess with kill_on_drop
- `setup_temp_file()` (helper, line 1456) — TempDir creation with `cognicode_rust_verify_` prefix

**DTOs** in `application/dto/file_ops.rs`:
- `VerificationStatus` enum — `Verified | Rejected | Skipped`
- `RetrieveAndVerifyRequest` — query, language, max_results, verify flag
- `RetrieveAndVerifyResult` — results vector with verified/rejected/skipped counts
- `VerifiedMatchDto` — per-match status with optional check_output/error_snippet

**MCP schemas** in `interface/mcp/schemas.rs`:
- `RetrieveAndVerifyInput` (line 1915) — mirrors the DTO with `verify` flag
- `RetrieveAndVerifyOutput` (line 1989) — same structure with its own `VerificationStatus` enum
- Registered as MCP tool `retrieve_and_verify` via `#[cognicode_macros::aix_tool]`

**Handler** in `interface/mcp/file_ops_handlers.rs` (line 278):
- `handle_retrieve_and_verify()` — converts MCP input → DTO → service call → MCP output
- Manual status enum mapping between DTO `VerificationStatus` and MCP `VerificationStatus`

**Tests** in `file_operations.rs` (lines 2843–3242):
- Empty query rejection
- No matches found
- Verify disabled
- Deterministic results
- Rust file verified (compiles)
- Rust file rejected (doesn't compile)
- `rustc` not found error message
- `kill_on_drop` semantics — orphan process detection

### What does NOT exist

- **No domain trait/port** for code verification. The capability is a concrete method, not behind
  an abstraction.
- **No `CompilationResult` type** — the sync method returns a 4-tuple `(VerificationStatus, Option<String>, Option<String>, Option<String>)`.
- **No `RustVerifier` struct or trait** anywhere in the codebase.
- **No separation of concerns** — `FileOperationsService` handles file reading, writing, editing,
  searching, listing, compression, syntax validation, AND rust verification (6+ responsibilities).

### The sandbox crate is NOT a verification sandbox

`cognicode-sandbox` (`crates/cognicode-sandbox/`) is a **scenario orchestrator** for MCP tool
testing. It:
- Loads YAML manifest files describing test scenarios
- Expands the language×tool×variant matrix
- Spawns the `cognicode-mcp` binary and executes tool calls
- Validates results against ground truth
- Scores scenarios on latency, scalability, consistency, robustness, correctness

It has a `ContainerConfig` for Docker/podman containers per language, but **zero rust verification
logic**. The sandbox crate is a test harness, not a sandboxed compilation environment.

---

## Context Quality

- **Level**: C2 — existing implementation present, need to extract/formalize
- **Evidence Present**:
  - `crates/cognicode-core/src/application/services/file_operations.rs` — full verification implementation with tests
  - `crates/cognicode-core/src/application/dto/file_ops.rs` — DTOs for verification
  - `crates/cognicode-core/src/interface/mcp/schemas.rs` — MCP input/output schemas
  - `crates/cognicode-core/src/interface/mcp/file_ops_handlers.rs` — handler wiring
  - `crates/cognicode-core/src/domain/traits/` — existing port catalog (no verifier trait)
  - `crates/cognicode-sandbox/src/main.rs` — scenario orchestrator (not relevant)
- **Missing Context**:
  - No ADR for the verification capability rationale
  - No documented decision on why verification lives in FileOperationsService vs. its own port
  - No target architecture diagram showing where verification should sit in hexagonal layers
- **Recommended Effort**: deepen — the implementation exists but needs architectural extraction

---

## Problem Taxonomy

| Axis | Applies | Evidence |
|------|---------|----------|
| Domain modeling | **Yes** | A `RustVerifier` port is missing from the domain layer. The concept of "compile this code safely" is a domain concern, not a file operation. |
| Boundary/seam | **Yes** | The boundary between file operations and code verification is currently invisible — both live in the same struct. A new trait creates the seam. |
| Coupling/connascence | **Yes** | `FileOperationsService` has 6+ responsibilities (SRP violation). Verification is tightly coupled to the service struct via `&self` methods even though it uses no service state beyond `rustc` availability. |
| API contract | **Yes** | A `RustVerifier` trait defines a contract that can be implemented by real-rustc, mock, or WASM backends. Currently untestable in isolation without real rustc. |
| Refactor/legacy | **Yes** | Extracting verification into a domain port is a refactoring. Existing tests provide a regression safety net. The handler must be updated if the service consumer changes. |
| Event/CQRS | No | Verification is synchronous request-response. No event sourcing or CQRS patterns apply. |
| Testing | **Yes** | Existing tests cover verification. Extraction to a trait enables injection of mock verifier for unit testing the handler without real rustc. |
| Security/operations | **Yes** | Sandboxed compilation involves temp directory management, subprocess lifecycle (kill_on_drop), timeout enforcement, and rustc binary availability checks — all operations concerns. |

---

## Domain Language And Invariants

- **Domain Language**:
  - `VerificationStatus` — `Verified`, `Rejected`, `Skipped` (already defined, used correctly)
  - `RetrieveAndVerify` — the combined search+verify operation (mixed concern; search and verify are separate capabilities)
  - `rustc --edition 2021 --crate-type lib` — the invariant compilation command
  - `CompilationResult` — **not yet modeled** as a type; currently a tuple `(status, stdout, stderr, reason)`
  - **Proposed additions**: `RustVerifier` (trait), `CompilationResult` (value object), `SandboxConfig` (value object for temp dir prefix, timeout, edition)

- **Invariants**:
  1. Verification MUST happen in a temp directory isolated from workspace files
  2. `rustc` MUST use `--crate-type lib` (library, not binary — no main required)
  3. Per-file timeout MUST be enforced (currently 10s)
  4. `rustc not found` MUST return a specific error, not a generic subprocess error
  5. Only `.rs` files are candidates for verification; others are `Skipped` with reason `not-rust`
  6. Temp directories use prefix `cognicode_rust_verify_`

- **Unresolved ambiguities**:
  - Should the trait be `RustVerifier` (language-specific) or `CodeVerifier` (language-agnostic with a `language` parameter)?
  - Should `CompilationResult` replace the 4-tuple? If so, where — DTO layer or domain layer?
  - Should the search+verify combination (`retrieve_and_verify`) remain as a composed service, or should search and verify be fully separated?

---

## Affected Areas

| Path | Why |
|------|-----|
| `crates/cognicode-core/src/domain/traits/` | New `code_verifier.rs` trait definition |
| `crates/cognicode-core/src/domain/traits/mod.rs` | Add `pub mod code_verifier` and re-export |
| `crates/cognicode-core/src/application/services/file_operations.rs` | Extract verification methods; `FileOperationsService` becomes a consumer of the trait |
| `crates/cognicode-core/src/application/dto/file_ops.rs` | Possibly add `CompilationResult` type; existing DTOs unchanged |
| `crates/cognicode-core/src/interface/mcp/file_ops_handlers.rs` | Minor update if `FileOperationsService` API changes |
| `crates/cognicode-core/src/infrastructure/` | New `verifier/` module with `RustcVerifier` implementation |
| `crates/cognicode-core/src/application/services/file_operations.rs` (tests) | Adapt tests for extracted verifier |

---

## Options

| Option | Pros | Cons | Effort |
|--------|------|------|--------|
| **A: Extract `RustVerifier` trait + `CompilationResult` type** | Clean hexagonal architecture; mockable for testing; follows existing trait pattern (`Parser`, `SearchProvider`); the domain vocabulary becomes explicit | Touches multiple files; requires test adaptation; need to decide trait granularity | Medium (2-4 hours) |
| **B: Extract only `CompilationResult` type, keep verification in FileOperationsService** | Minimal change; reduces the 4-tuple smell; adds domain vocabulary | Does not fix SRP violation; verification still untestable in isolation; no seam for future WASM or remote verifiers | Low (30 min) |
| **C: Create separate `RustVerifierService` without trait** | Less ceremony than a trait; separates SRP concern | No abstraction for mocking; `FileOperationsService` still depends on concrete impl; violates DIP | Medium (1-2 hours) |
| **D: Do nothing** | Zero effort; existing code works and is tested | SRP violation grows; harder to test; no seam for future verifier backends; domain vocabulary remains implicit | Zero |

---

## Entropy Envelope

- **Method**: heuristic (CogniCode graph unavailable for method-level analysis in this pass; build_graph cancelled)
- **Coupling risk**: medium

### SRP Free Energy Analysis

`FileOperationsService` currently has ~20 public/private methods spanning:
- File reading (raw, outline, symbols, compressed, chunked)
- File writing (atomic)
- File editing (string replacement + tree-sitter validation)
- Content searching (regex/literal, gitignore-aware)
- File listing (directory walk, gitignore-aware)
- **Rust verification** (sync, async, timeout)
- Content compression
- Path validation and security checks

```
H(methods) ≈ log2(20) ≈ 4.32 bits
H(methods | purpose="file operations") ≈ log2(~17) ≈ 4.09 bits  
  (verification methods are ~3 out of ~20)
F = 4.32 - 4.09 ≈ 0.23 bits → LOW
```

The free energy is low, meaning most methods ARE explainable by "file operations". However,
verification methods (`verify_rust_file`, `verify_rust_file_with_timeout`, `run_rustc_async`,
`setup_temp_file`, `retrieve_and_verify`) share a distinct purpose: "compile this code safely".

If split into `FileOperationsService` (file ops) + `RustVerifier` (compilation):
```
F_file_ops = log2(17) - log2(17) ≈ 0 bits → clean
F_verifier = log2(5) - log2(5) ≈ 0 bits → clean
F_combined = 0 + 0 = 0 < 0.23 → split is justified
```

### Connascence Landscape

| Component A | Component B | Type | I(bits) | Severity | Hidden? |
|-------------|-------------|------|---------|----------|---------|
| FileOperationsService | rustc binary | Name | 1.0 | ⚠️ Low | No |
| FileOperationsService | RetrieveAndVerifyRequest (DTO) | Type | 1.58 | ⚠️ Medium | No |
| FileOperationsService | VerificationStatus (DTO) | Type | 1.0 | ⚠️ Low | No |
| FileOperationsService | MCP handler (via retrieve_and_verify) | Algorithm | 1.58 | ⚠️ Medium | No |

**No critical pairs** (I > 3.0 bits). Coupling is manageable.
**No hidden connascence** detected — all dependencies are explicit imports.

### SOLID-Entropy

- **SRP**: ⚠️ Violated — `FileOperationsService` mixes file ops + verification. F_before > F_after justifies split.
- **OCP**: ✅ Satisfied — verification is a private implementation detail, not extended. Extracting to trait would make it OCP-compliant (new verifier backends via trait impl).
- **LSP**: N/A — no subtyping involved.
- **ISP**: ⚠️ Potential — if `RustVerifier` trait is too broad (e.g., includes search+verify), clients only wanting compilation get unnecessary search dependency.
- **DIP**: ❌ Violated — the handler depends on `FileOperationsService` (concrete), not a verifier abstraction. After extraction: handler depends on `RustVerifier` trait (abstract).

---

## Recommendation

**Option A: Extract `RustVerifier` trait + `CompilationResult` value object.**

This is the right architectural move because:

1. **Hexagonal alignment**: The project uses Clean/Hexagonal Architecture with domain ports
   (`Parser`, `SearchProvider`, `GraphQueryPort`, etc.). A `RustVerifier` port follows the
   established pattern exactly.

2. **SRP restoration**: `FileOperationsService` is doing too much. Extracting verification to
   its own port gives each component a single reason to change.

3. **Testability**: Currently, testing the handler requires a real `rustc` binary. A trait
   enables injection of a `MockVerifier` for pure unit tests.

4. **Future seams**: The `Context7` integration and future WASM compilation backends need a
   trait to implement against. Without it, we'd add more methods to `FileOperationsService`.

5. **Low blast radius**: The change affects ~7 files, all within `cognicode-core`. No external
   crates break. Existing tests serve as regression safety net.

### Proposed shape

```rust
// domain/traits/code_verifier.rs
pub trait CodeVerifier: Send + Sync {
    /// Verify a single file compiles. Returns a CompilationResult.
    fn verify(&self, file_path: &Path) -> AppResult<CompilationResult>;

    /// Verify with a deadline. Returns error if timeout exceeded.
    async fn verify_with_timeout(
        &self,
        file_path: &Path,
        timeout: Duration,
    ) -> AppResult<CompilationResult>;
}

// domain/value_objects/compilation_result.rs
pub struct CompilationResult {
    pub status: VerificationStatus,
    pub stdout: Option<String>,
    pub stderr_snippet: Option<String>,
    pub reason: Option<String>,
}
```

The trait should be called `CodeVerifier` (not `RustVerifier`) to allow future language-agnostic
implementations, but the first impl is `RustcVerifier`. The `retrieve_and_verify` composed
operation stays in `FileOperationsService` as an orchestrator that uses both `SearchProvider`
and `CodeVerifier`.

### ISP note

Keep the trait narrow: `verify` + `verify_with_timeout`. Do NOT include `search` or the combined
`retrieve_and_verify` operation. The composed operation belongs in the application service layer.

---

## Ready For Proposal

**Yes** — all evidence is gathered. The domain vocabulary is clear. The blast radius is
quantified. The architecture choice (Option A) has clear tradeoffs. Proceed to `sdd-propose`
with:
- Extract `CodeVerifier` trait in `domain/traits/`
- Add `CompilationResult` value object in `domain/value_objects/`
- Create `RustcVerifier` in `infrastructure/verifier/`
- Wire through `FileOperationsService` as constructor-injected dependency
- Update handler to use the trait
- Adapt existing tests
