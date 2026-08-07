# Fixture Specification: ts-zod (colinhacks/zod)

## Purpose

Add a real-world TypeScript schema validation library as a Tier B sandbox
fixture, complementing commander.js (a CLI framework). zod uses advanced
TypeScript patterns: discriminated unions, template literal types, conditional
types, and complex generic inference — stress-testing the TS analysis pipeline.

## Fixture Metadata

| Field       | Value                                          |
|-------------|------------------------------------------------|
| Repo        | colinhacks/zod                                 |
| Language    | TypeScript                                     |
| Tier        | B (nightly, not smoke lane)                    |
| Pin target  | Latest stable tag (e.g. v3.x)                 |
| Path        | `sandbox/repos/typescript/zod/`                |
| Manifest    | `sandbox/manifests/ts_repos.yaml`              |
| Size budget | ≤ 5 MB on disk (excluding `node_modules`)     |

## Requirements

### Requirement: Repo Snapshot Presence

The fixture MUST contain a pinned snapshot of colinhacks/zod at a tagged SHA.
At minimum: `src/types.ts`, `src/index.ts`, `tsconfig.json`, and `package.json`
MUST exist.

#### Scenario: Core TypeScript source files are present after setup

- GIVEN the sandbox setup script has run for ts-zod
- WHEN `sandbox/repos/typescript/zod/` is inspected
- THEN `src/types.ts`, `src/index.ts`, `tsconfig.json`, and `package.json` MUST exist
- AND `node_modules/` MUST NOT be committed into the snapshot

#### Scenario: Pinned SHA is reproducible

- GIVEN the manifest entry specifies a SHA
- WHEN the setup script clones or restores the fixture
- THEN the resulting HEAD commit MUST match the pinned SHA exactly

---

### Requirement: Manifest Entry

A valid manifest entry MUST exist for ts-zod in `ts_repos.yaml`, containing:
`language`, `repo_url`, `pinned_sha`, `description`, and `tier`.

#### Scenario: Manifest entry validates against schema

- GIVEN `sandbox/manifests/ts_repos.yaml` contains the ts-zod entry
- WHEN the manifest schema validator runs
- THEN validation MUST pass with zero errors

---

### Requirement: Validation Stages

The ts-zod fixture MUST define these validation stages:

| Stage      | Command                                             | Timeout |
|------------|-----------------------------------------------------|---------|
| install    | `npm ci --frozen-lockfile \|\| npm install`          | 120s    |
| typecheck  | `node_modules/.bin/tsc --noEmit`                    | 60s     |
| test       | `node_modules/.bin/jest --passWithNoTests`          | 120s    |

#### Scenario: npm install succeeds

- GIVEN the fixture is present and Node.js ≥18 is available
- WHEN `npm ci` or `npm install` is run in `sandbox/repos/typescript/zod/`
- THEN exit code MUST be 0

#### Scenario: TypeScript type check passes

- GIVEN the fixture is present and dependencies are installed
- WHEN `tsc --noEmit` is run
- THEN exit code MUST be 0 with no type errors

#### Scenario: Tests pass

- GIVEN the fixture is present and dependencies are installed
- WHEN `jest` is run
- THEN exit code MUST be 0

---

### Requirement: CogniCode TypeScript Advanced-Types Compatibility

CogniCode's pipeline MUST handle zod's advanced TypeScript patterns without
errors: discriminated unions, conditional types, and deep generic inference.

#### Scenario: read_file on types.ts

- GIVEN ts-zod fixture is present
- WHEN `read_file(path="src/types.ts", mode="raw")` is called
- THEN result MUST contain TypeScript source text with `pass` outcome

#### Scenario: search_content finds discriminated union pattern

- GIVEN ts-zod fixture is present
- WHEN `search_content(query="discriminated")` or `search_content(query="ZodDiscriminated")` is called
- THEN at least one match MUST be returned

#### Scenario: extract_symbols handles conditional type aliases

- GIVEN ts-zod fixture is present and contains complex type aliases
- WHEN `extract_symbols(path="src/types.ts")` is called
- THEN result MUST be `pass` or `capability_missing` — NEVER `error`
- AND if `pass`, type aliases MUST appear in the symbol list

#### Scenario: Deep generic inference does not cause timeout or panic

- GIVEN ts-zod fixture contains files with 5+ levels of generic nesting
- WHEN any analysis tool processes such a file
- THEN the call MUST complete within the manifest `timeout_seconds`
- AND result MUST NOT be `error` or contain a stack trace

#### Scenario: Template literal types survive analysis

- GIVEN ts-zod contains template literal type definitions
- WHEN `extract_symbols` or `search_content` is invoked on such a file
- THEN result MUST be `pass` or `capability_missing` — NEVER `error`

---

## Correctness Metrics

| Metric                              | Target    |
|-------------------------------------|-----------|
| Manifest schema validation          | 100% pass |
| `tsc --noEmit` success              | 100%      |
| `jest` test suite success           | 100%      |
| Analysis pipeline errors            | 0         |
| Analysis panics/timeouts            | 0         |
| Fixture size (no node_modules)      | ≤ 5 MB    |
