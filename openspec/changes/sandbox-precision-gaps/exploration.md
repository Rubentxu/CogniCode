## Exploration: sandbox-precision-gaps

### Current State
The remaining failures are not one bug. They are four distinct classes of
precision gaps across manifest parsing, repo/workspace selection, tool-schema
mapping, and TypeScript edit validation.

- **Top-level `repo:` in `rust.yaml` and `python.yaml` is ignored.**
  `src/sandbox_core/manifest.rs:14-34` does not define `Manifest.repo`, so
  manifest line 18 in both files is silently dropped during YAML parse.
  Only scenario-level `repo` exists at `src/sandbox_core/manifest.rs:56-59`,
  and only that field is propagated at `:183`.

- **The orchestrator does not clone repos at all.**
  `src/bin/sandbox_orchestrator.rs:291-346` only resolves paths under
  `repos_dir`; clone/update logic lives separately in
  `sandbox/scripts/clone_repos.sh:13-85`. Existing clones are present at
  `sandbox/repos/serde` and `sandbox/repos/click`.

- **When `scenario.repo` is `None`, fixtures win over repos.**
  In `src/bin/sandbox_orchestrator.rs:335-346`, `use_fixture` becomes true for
  Rust/Python because `sandbox/fixtures/rust` and `sandbox/fixtures/python`
  exist, so the cloned repos are ignored even though `sandbox/repos/serde` and
  `sandbox/repos/click` exist.

- **If a scenario-level `repo` were set to a GitHub URL, it would still fail.**
  `repo_name = scenario.repo.clone()` at
  `src/bin/sandbox_orchestrator.rs:314-326`, then
  `repo_path = repos_dir.join(&repo_name)` at `:327`. A URL would become an
  invalid local path such as
  `sandbox/repos/https://github.com/serde-rs/serde`.

- **Rust manifest paths are wrong for the real serde repo.**
  `sandbox/repos/serde` is a Cargo workspace (`sandbox/repos/serde/Cargo.toml`
  lines 1-10). The real crate is under `sandbox/repos/serde/serde/`, and the
  relevant file is `sandbox/repos/serde/serde/src/ser/mod.rs` where
  `pub trait Serialize` is at line 218 and `pub trait Serializer` is at line
  333. The manifest points to `src/ser.rs`, which does not exist in the repo.

- **Python manifest paths are correct for the real click repo, but the chosen
  workspace is wrong.**
  `sandbox/repos/click/src/click/__init__.py` exists and exports `Command` at
  line 9, `command` at line 17, and `group` at line 19. The current failure is
  not path layout inside the repo; it is that the orchestrator starts the MCP
  server with a nonexistent fixture-derived cwd.

- **Rust `extract_symbols` and `find_references` also have tool-contract bugs.**
  The MCP server exposes `get_file_symbols`, not `extract_symbols`
  (`src/interface/mcp/server.rs:924-938`). `find_references` requires
  `file_path`, `line`, and `column`
  (`src/interface/mcp/server.rs:742-753`,
  `src/interface/mcp/schemas.rs:929-937`), but the manifest sends `path` and
  `symbol`.

- **TypeScript is no longer blocked by the symlink copy bug.**
  `copy_dir_recursive()` preserves symlinks at
  `src/bin/sandbox_orchestrator.rs:355-390`. The source fixtures still contain
  symlinked `node_modules/.bin/tsc`:
  `sandbox/fixtures/typescript/ts-mutation/node_modules/.bin/tsc -> ../typescript/bin/tsc`
  and the latest validation passes `npx tsc --noEmit`.

- **The remaining TypeScript real failure is an MCP edit validation failure,
  not a `tsc` failure.**
  `typescript_edit_file_concrete_concrete` now fails with
  `response.json` showing `{"applied":false,"validation":{"passed":false...}}`
  while validation log shows both typecheck and tests passing. The root cause is
  `src/infrastructure/parser/tree_sitter_parser.rs:47-53`, where
  `Language::TypeScript` incorrectly uses the JavaScript grammar
  `tree_sitter_javascript::LANGUAGE`, so `edit_file` syntax validation rejects
  valid TypeScript edits before writing them.

#### Exact remaining Rust failures (4 requested)

1. **`rust_read_file_raw_default`**
   - Manifest: `sandbox/manifests/rust.yaml:45-55`
   - Tested operation: `read_file(path="src/ser.rs", mode="raw")`
   - Request artifact:
     `sandbox/results/rust_read_file_raw_default/20260410T164002/request.json:4-9`
   - Actual error:
     `sandbox/results/rust_read_file_raw_default/20260410T164002/response.json:7`
     → `Application error: Internal error: Invalid parameter: Failed to read metadata: No such file or directory (os error 2)`
   - Actual cwd sent to MCP: fixture temp root (because read-only file workspace
     `src/ser.rs` is collapsed to temp fixture root by
     `src/bin/sandbox_orchestrator.rs:453-458`), not the serde repo.
   - Repo cloned? Yes, but unused: `sandbox/repos/serde` exists.
   - Exact root cause: top-level repo ignored + fixture precedence + wrong file
     path for real serde layout.
   - Exact fix: add top-level manifest repo support or set scenario-level repo,
     prefer repo over fixture when repo is declared, and change workspace/path to
     `workspace: serde`, `path: src/ser/mod.rs`.

2. **`rust_read_file_outline_default`**
   - Manifest: `sandbox/manifests/rust.yaml:56-66`
   - Tested operation: `read_file(path="src/ser.rs", mode="outline")`
   - Actual error:
     `sandbox/results/rust_read_file_outline_default/20260410T164002/response.json:7`
     → same `Failed to read metadata: No such file or directory (os error 2)`
   - Actual cwd sent to MCP: same wrong temp fixture root.
   - Repo cloned? Yes, but unused.
   - Exact root cause: identical to raw read.
   - Exact fix: same as raw read.

3. **`rust_extract_symbols_default`**
   - Manifest: `sandbox/manifests/rust.yaml:80-97`
   - Intended operation: symbol extraction from `src/ser.rs`
   - Actual result:
     `sandbox/results/rust_extract_symbols_default/20260410T164002/result.json:10-35`
     → outcome `syntax_failure`
   - Actual validation error:
     `sandbox/results/rust_extract_symbols_default/20260410T164002/validation.log:9-10`
     → `Error: file \`src/\` does not exist`
   - Actual cwd sent to MCP/validation: `/tmp/.../src` parent dir of file-like
     workspace because analysis scenarios use parent directory at
     `src/bin/sandbox_orchestrator.rs:459-469`; then validation runs
     `rustfmt --check src/` from `/tmp/.../src`, which resolves to nonexistent
     `/tmp/.../src/src`.
   - Secondary manifest bug: tool name `extract_symbols` is wrong; MCP only
     exposes `get_file_symbols` (`src/interface/mcp/server.rs:924-938`).
   - Exact root cause: validation cwd bug masks an unsupported tool name.
   - Exact fix: rename tool to `get_file_symbols`, send `file_path`, and run the
     scenario from crate root `workspace: serde` so validation executes in the
     correct directory.

4. **`rust_find_references_default`**
   - Manifest: `sandbox/manifests/rust.yaml:98-107`
   - Tested operation: `find_references` on file `src/ser.rs`
   - Request artifact:
     `sandbox/results/rust_find_references_default/20260410T164002/request.json:4-8`
     sends `{ action: "references", path: "src/ser.rs", symbol: "Serialized" }`
   - Actual MCP error:
     `sandbox/results/rust_find_references_default/20260410T164002/response.json:7`
     → `Invalid input: Invalid find_references input: missing field \`file_path\``
   - Secondary validation error:
     `sandbox/results/rust_find_references_default/20260410T164002/validation.log:9-10`
     → `Error: file \`src/\` does not exist`
   - Exact root cause: manifest schema is incompatible with MCP; `find_references`
     requires `file_path`, `line`, and `column`, not `path` and `symbol`, and
     the analysis cwd bug adds a second failure.
   - Exact fix: change arguments to MCP schema and run from a real serde crate
     root/file that actually exists.

#### Exact remaining Python failures (4 requested)

1. **`python_read_file_raw_default`**
   - Manifest: `sandbox/manifests/python.yaml:45-55`
   - Tested operation: `read_file(path="src/click/__init__.py", mode="raw")`
   - Actual result:
     `sandbox/results/python_read_file_raw_default/20260410T164010/result.json:10-18`
     → `outcome: "no_result"`, `server_startup_ms: 0`
   - Actual cwd that the orchestrator tries to use: nonexistent fixture path
     `/tmp/.../src/click`, because `workspace: src/click` is treated as a
     directory and appended to the copied Python fixture at
     `src/bin/sandbox_orchestrator.rs:471-473`.
   - Repo cloned? Yes: `sandbox/repos/click` exists.
   - Does MCP receive the correct workspace directory? No.
   - Are manifest paths correct for real repo? Yes,
     `sandbox/repos/click/src/click/__init__.py` exists.
   - Exact root cause: top-level repo ignored + fixture precedence + invalid cwd.
   - Exact fix: run from repo root (`workspace: .` or scenario-level repo root),
     not `src/click`, and honor the manifest repo.

2. **`python_read_file_outline_default`**
   - Manifest: `sandbox/manifests/python.yaml:56-66`
   - Tested operation: `read_file(path="src/click/__init__.py", mode="outline")`
   - Actual result:
     `sandbox/results/python_read_file_outline_default/20260410T164010/result.json:10-18`
     → `outcome: "no_result"`, `server_startup_ms: 0`
   - Exact root cause: same nonexistent cwd `/tmp/.../src/click`.
   - Exact fix: same as raw read.

3. **`python_search_content_default`**
   - Manifest: `sandbox/manifests/python.yaml:67-77`
   - Tested operation: `search_content(pattern="Command", path="src/click")`
   - Actual result:
     `sandbox/results/python_search_content_default/20260410T164010/result.json:10-18`
     → `outcome: "no_result"`, `server_startup_ms: 0`
   - Exact root cause: same nonexistent cwd `/tmp/.../src/click`; server never
     initializes, so the tool is never called.
   - Exact fix: same repo/workspace fix.

4. **`python_extract_symbols_default`**
   - Manifest: `sandbox/manifests/python.yaml:80-97`
   - Intended operation: symbol extraction from `src/click/__init__.py`
   - Actual result:
     `sandbox/results/python_extract_symbols_default/20260410T164010/result.json:10-34`
     → `outcome: "syntax_failure"`
   - Actual error:
     `sandbox/results/python_extract_symbols_default/20260410T164010/validation.log:6-7`
     → `No such file or directory (os error 2)`
   - Current root cause: validation is started with nonexistent cwd because MCP
     never started.
   - Secondary manifest bug: tool name `extract_symbols` is wrong; MCP exposes
     `get_file_symbols`.
   - Exact fix: first fix repo/workspace selection; then rename tool to
     `get_file_symbols` and pass `file_path`.

#### Exact remaining TypeScript real failure (1 requested)

1. **`typescript_edit_file_concrete_concrete`**
   - Manifest: `sandbox/manifests/ts.yaml:84-105`
   - Tested operation: rename function signature text in
     `sandbox/fixtures/typescript/ts-mutation/index.ts:6`
   - Request artifact:
     `sandbox/results/typescript_edit_file_concrete_concrete/20260410T164107/request.json:4-12`
   - Actual result:
     `sandbox/results/typescript_edit_file_concrete_concrete/20260410T164107/result.json:10-40`
     → `outcome: "edit_rejected"`
   - Actual MCP response:
     `sandbox/results/typescript_edit_file_concrete_concrete/20260410T164107/response.json:7`
     → `{"applied":false,"validation":{"passed":false,"syntax_errors":[]},"preview":"Changed 0 bytes","bytes_changed":0}`
   - Validation after the tool call passes:
     `sandbox/results/typescript_edit_file_concrete_concrete/20260410T164107/validation.log:1-9`
     shows `typecheck` and `test` both pass.
   - Is it the symlink issue? **No.** The old symlink problem is gone; latest
     `list_files` also passes, and typecheck succeeds.
   - Does `node_modules/.bin/tsc` work after symlink-aware copy? **Yes in
     effect**: `npx tsc --noEmit` passes in the copied temp workspace, which
     would not happen if `.bin/tsc` were still broken.
   - Exact root cause: `edit_file` validates TypeScript with the JavaScript
     grammar at `src/infrastructure/parser/tree_sitter_parser.rs:47-53`, so it
     rejects syntactically valid TypeScript edits before writing them.
   - Exact fix: use a real TypeScript grammar/parser for
     `Language::TypeScript`, then keep the current symlink-preserving copy code.

### Affected Areas
- `sandbox/manifests/rust.yaml` — wrong serde repo paths, wrong tool name, wrong
  `find_references` schema.
- `sandbox/manifests/python.yaml` — correct click file paths, but wrong workspace
  semantics and wrong symbol-extraction tool name.
- `sandbox/manifests/ts.yaml` — remaining concrete failure is parser validation,
  not workspace copying.
- `src/sandbox_core/manifest.rs` — top-level manifest `repo` is unsupported.
- `src/bin/sandbox_orchestrator.rs` — fixture precedence, repo path resolution,
  cwd derivation, and absence of clone logic.
- `src/interface/mcp/server.rs` — canonical MCP tool names and required schemas.
- `src/interface/mcp/schemas.rs` — canonical input fields for file/symbol tools.
- `src/infrastructure/parser/tree_sitter_parser.rs` — TypeScript incorrectly
  mapped to JavaScript grammar.
- `sandbox/scripts/clone_repos.sh` — real clone/update logic lives here, not in
  the orchestrator.

### Approaches
1. **Fix manifest/repo semantics first** — teach manifest parsing and the
   orchestrator to honor real repos and use the right cwd.
   - Pros: Unblocks Rust and Python immediately.
   - Cons: Requires touching manifest parsing plus orchestrator path logic.
   - Effort: Medium.

2. **Fix tool contract mismatches next** — rename manifest tools and arguments to
   match MCP schemas.
   - Pros: Removes masked false negatives in Rust/Python analysis scenarios.
   - Cons: Requires per-scenario manifest edits.
   - Effort: Low.

3. **Fix TypeScript parser validation** — use a TypeScript grammar instead of the
   JavaScript grammar for edit validation.
   - Pros: Resolves the last real TS failure without reverting symlink work.
   - Cons: Needs parser wiring/tests.
   - Effort: Medium.

### Recommendation
Do the change in this order:

1. Add top-level `repo` support in `Manifest`, or propagate manifest-level repo
   into every scenario.
2. In `execute_scenario()`, prefer repo workspaces when repo is declared, and do
   not treat raw GitHub URLs as local folder names.
3. Fix Rust manifest to target the actual serde crate layout:
   `workspace: serde`, `path: src/ser/mod.rs`.
4. Fix Python manifest to run from repo root, keeping file paths under `src/`.
5. Rename `extract_symbols` → `get_file_symbols` and fix
   `find_references` arguments to `{file_path,line,column}`.
6. Replace the TypeScript parser grammar mapping so `edit_file` accepts valid TS.

### Risks
- Enabling manifest-level repo support will change current fixture fallback
  behavior for any manifests that were accidentally relying on it.
- Fixing Rust to use the real serde crate layout will change validation cwd and
  may require updating `cargo` commands accordingly.
- Switching TypeScript to a true TS grammar can change current symbol-extraction
  behavior and should be regression-tested for JS.

### Ready for Proposal
Yes — the remaining failures are now fully localized and each one has a concrete
code-level fix path.
