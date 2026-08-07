# Fixture Specification: go-lo (samber/lo)

## Purpose

Provide a real-world Go generic utility library as a Tier B sandbox fixture.
`lo` exercises Go 1.18+ generics heavily — type parameters, constraints, and
generic function signatures — providing coverage that cobra and bubbletea do not.

## Fixture Metadata

| Field       | Value                                    |
|-------------|------------------------------------------|
| Repo        | samber/lo                                |
| Language    | Go                                       |
| Tier        | B (nightly, not smoke lane)              |
| Pin target  | Latest stable tag (e.g. v1.47.x)        |
| Path        | `sandbox/repos/go/lo/`                   |
| Manifest    | `sandbox/manifests/go_repos.yaml`        |
| Size budget | ≤ 3 MB on disk                           |

## Requirements

### Requirement: Repo Snapshot Presence

The fixture MUST contain a pinned snapshot of samber/lo at a tagged release SHA.
The repo MUST include `lo.go` or equivalent entry files and `go.mod` specifying
`go 1.18` or higher.

#### Scenario: Core source files present after setup

- GIVEN the sandbox setup script has run for go-lo
- WHEN `sandbox/repos/go/lo/` is inspected
- THEN `go.mod` MUST exist and declare `go 1.18` or higher
- AND at least 10 `.go` source files MUST be present

#### Scenario: Pinned SHA is reproducible

- GIVEN the manifest entry for go-lo specifies a SHA
- WHEN the setup script clones or restores the fixture
- THEN the resulting HEAD commit MUST match the pinned SHA exactly

---

### Requirement: Manifest Entry

A valid manifest entry MUST exist for go-lo in `go_repos.yaml`, containing:
`language`, `repo_url`, `pinned_sha`, `description`, and `tier`.

#### Scenario: Manifest entry validates against schema

- GIVEN `sandbox/manifests/go_repos.yaml` contains the go-lo entry
- WHEN the manifest schema validator runs
- THEN validation MUST pass with zero errors

---

### Requirement: Validation Stages

The go-lo fixture MUST define these validation stages:

| Stage   | Command          | Timeout |
|---------|-----------------|---------|
| syntax  | `go build ./...` | 60s     |
| vet     | `go vet ./...`   | 60s     |
| test    | `go test ./...`  | 180s    |

#### Scenario: go build succeeds with generics

- GIVEN the fixture is present and `go` ≥1.21 is available
- WHEN `go build ./...` is run
- THEN exit code MUST be 0

#### Scenario: go test passes

- GIVEN the fixture is present
- WHEN `go test ./...` is run
- THEN exit code MUST be 0 and test count MUST be ≥ 20

---

### Requirement: CogniCode Generics Compatibility

CogniCode's analysis pipeline MUST NOT error on Go generic type parameters.
`lo`'s generic functions are the primary stress test for this requirement.

#### Scenario: read_file on generic function file

- GIVEN go-lo fixture is present
- WHEN `read_file` is called on a file containing `[T any]` type parameters
- THEN result MUST contain source text with `pass` outcome

#### Scenario: search_content finds generic constraint

- GIVEN go-lo fixture is present
- WHEN `search_content(query="[T any]")` is called
- THEN at least one match MUST be returned

#### Scenario: extract_symbols handles generic function signatures

- GIVEN go-lo fixture is present
- WHEN `extract_symbols` is called on a file with generic functions
- THEN result MUST return `pass` or `capability_missing` — NEVER `error`
- AND any extracted symbols MUST preserve the function name without mangling

#### Scenario: Analysis does not panic on type constraint syntax

- GIVEN go-lo fixture is present and a file contains `comparable` or `constraints.Ordered`
- WHEN any analysis tool processes that file
- THEN the pipeline MUST not panic or crash (exit code ≠ 2 / SIGSEGV)

---

## Correctness Metrics

| Metric                              | Target    |
|-------------------------------------|-----------|
| Manifest schema validation          | 100% pass |
| `go build ./...` success            | 100%      |
| Analysis pipeline panics            | 0         |
| Generic symbol extraction errors    | 0         |
| Fixture size on disk                | ≤ 3 MB    |
