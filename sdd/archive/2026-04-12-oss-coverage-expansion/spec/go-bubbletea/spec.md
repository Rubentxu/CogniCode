# Fixture Specification: go-bubbletea (charmbracelet/bubbletea)

## Purpose

Provide a real-world Go TUI framework repo as a Tier B sandbox fixture.
bubbletea uses the Elm architecture in Go, featuring interfaces, functional
patterns, and embedded types — providing complementary coverage to cobra's
CLI command patterns.

## Fixture Metadata

| Field       | Value                                          |
|-------------|------------------------------------------------|
| Repo        | charmbracelet/bubbletea                        |
| Language    | Go                                             |
| Tier        | B (nightly, not smoke lane)                    |
| Pin target  | Latest stable tag (e.g. v0.27.x or v1.x)      |
| Path        | `sandbox/repos/go/bubbletea/`                  |
| Manifest    | `sandbox/manifests/go_repos.yaml`              |
| Size budget | ≤ 5 MB on disk                                 |

## Requirements

### Requirement: Repo Snapshot Presence

The fixture MUST contain a pinned snapshot of charmbracelet/bubbletea at a
tagged release SHA. The repo MUST include at minimum: `tea.go` (or `bubbletea.go`),
`key.go`, `msg.go`, and `go.mod`.

#### Scenario: Core source files are present after setup

- GIVEN the sandbox setup script has run for go-bubbletea
- WHEN `sandbox/repos/go/bubbletea/` is inspected
- THEN `go.mod`, `go.sum`, and at least one `.go` root file MUST exist
- AND the Go module path in `go.mod` MUST contain `bubbletea`

#### Scenario: Pinned SHA is reproducible

- GIVEN the manifest entry for go-bubbletea specifies a SHA
- WHEN the setup script clones or restores the fixture
- THEN the resulting HEAD commit MUST match the pinned SHA exactly

---

### Requirement: Manifest Entry

A valid manifest entry MUST exist for go-bubbletea in `go_repos.yaml`,
containing: `language`, `repo_url`, `pinned_sha`, `description`, and `tier`.

#### Scenario: Manifest entry validates against schema

- GIVEN `sandbox/manifests/go_repos.yaml` exists and contains the go-bubbletea entry
- WHEN the manifest schema validator runs
- THEN validation MUST pass with zero errors for the go-bubbletea entry

---

### Requirement: Validation Stages

The go-bubbletea fixture MUST define the following validation stages:

| Stage   | Command          | Timeout |
|---------|-----------------|---------|
| syntax  | `go build ./...` | 60s     |
| vet     | `go vet ./...`   | 60s     |
| test    | `go test ./...`  | 180s    |

#### Scenario: go build passes on bubbletea fixture

- GIVEN the fixture is present and `go` ≥1.21 is available
- WHEN `go build ./...` is run in `sandbox/repos/go/bubbletea/`
- THEN exit code MUST be 0

#### Scenario: Interface types survive go vet

- GIVEN the fixture is present
- WHEN `go vet ./...` is run
- THEN exit code MUST be 0 with no vet warnings

---

### Requirement: CogniCode Analysis Compatibility

CogniCode's pipeline MUST handle bubbletea's interface-heavy patterns without
errors. The `Model` interface and `Update`/`View` method signatures are key
analysis targets.

#### Scenario: read_file on main tea source

- GIVEN go-bubbletea fixture is present
- WHEN `read_file` is called on the root `.go` entry file
- THEN result MUST contain Go source text with `pass` outcome

#### Scenario: search_content finds Model interface

- GIVEN go-bubbletea fixture is present
- WHEN `search_content(query="type Model interface")` is called
- THEN at least one match MUST be returned

#### Scenario: extract_symbols returns interface declarations

- GIVEN go-bubbletea fixture is present
- WHEN `extract_symbols` is called on the root entry file
- THEN result MUST include at least one symbol of kind `interface` or `type`

#### Scenario: Analysis handles embedded types without panic

- GIVEN go-bubbletea fixture is present and uses embedded struct patterns
- WHEN any analysis tool is invoked on a file with embedded types
- THEN result MUST be `pass` or `capability_missing` — NEVER `error` or panic

---

## Correctness Metrics

| Metric                         | Target    |
|--------------------------------|-----------|
| Manifest schema validation     | 100% pass |
| `go build ./...` success       | 100%      |
| Analysis pipeline errors       | 0         |
| Interface extraction accuracy  | ≥ 1 symbol per interface file |
| Fixture size on disk           | ≤ 5 MB    |
