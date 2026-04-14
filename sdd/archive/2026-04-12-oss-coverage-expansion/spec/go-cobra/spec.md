# Fixture Specification: go-cobra (spf13/cobra)

## Purpose

Provide a real-world Go CLI framework repo as a Tier B sandbox fixture,
establishing Go as a first-class language in CogniCode's multi-language
benchmark matrix. cobra is a mid-sized, widely-used OSS project with rich
package structure, interfaces, and CLI patterns.

## Fixture Metadata

| Field       | Value                                        |
|-------------|----------------------------------------------|
| Repo        | spf13/cobra                                  |
| Language    | Go                                           |
| Tier        | B (nightly, not smoke lane)                  |
| Pin target  | Latest stable tag (e.g. v1.8.x)             |
| Path        | `sandbox/repos/go/cobra/`                    |
| Manifest    | `sandbox/manifests/go_repos.yaml`            |
| Size budget | ≤ 5 MB on disk (sparse checkout if needed)  |

## Requirements

### Requirement: Repo Snapshot Presence

The fixture MUST contain a pinned, reproducible snapshot of spf13/cobra at a
tagged release SHA. The repo MUST include at minimum: `cobra.go`, `command.go`,
`completions.go`, and `go.mod`.

#### Scenario: Fixture files are present after setup

- GIVEN the sandbox setup script has run for go-cobra
- WHEN the filesystem path `sandbox/repos/go/cobra/` is inspected
- THEN `cobra.go`, `command.go`, `go.mod`, and `go.sum` MUST exist
- AND no file SHOULD exceed 500 KB individually

#### Scenario: Pinned SHA is reproducible

- GIVEN the manifest entry for go-cobra specifies a SHA
- WHEN the setup script clones or restores the fixture
- THEN the resulting HEAD commit MUST match the pinned SHA exactly

---

### Requirement: Manifest Entry

A valid `manifest.json` (or YAML entry in `go_repos.yaml`) MUST exist for
go-cobra, containing: `language`, `repo_url`, `pinned_sha`, `description`,
and `tier`.

#### Scenario: Manifest is schema-valid

- GIVEN the manifest file exists at `sandbox/manifests/go_repos.yaml`
- WHEN the manifest schema validator runs
- THEN validation MUST pass with zero errors

---

### Requirement: Validation Stages

The go-cobra fixture MUST define the following validation stages in its manifest:

| Stage    | Command                         | Timeout |
|----------|---------------------------------|---------|
| syntax   | `go build ./...`                | 60s     |
| vet      | `go vet ./...`                  | 60s     |
| test     | `go test ./...`                 | 180s    |

#### Scenario: go build passes on fixture

- GIVEN the go-cobra fixture is present and `go` ≥1.21 is available
- WHEN `go build ./...` is run in `sandbox/repos/go/cobra/`
- THEN exit code MUST be 0

#### Scenario: go test passes on fixture

- GIVEN the go-cobra fixture is present
- WHEN `go test ./...` is run
- THEN exit code MUST be 0 and test count MUST be ≥ 10

---

### Requirement: CogniCode Analysis Compatibility

CogniCode's analysis pipeline MUST run without errors on go-cobra. Scenarios
MUST cover: read_file, search_content, extract_symbols, and safe_refactor.

#### Scenario: read_file on Go source succeeds

- GIVEN go-cobra fixture is present
- WHEN `read_file(path="cobra.go", mode="raw")` is called
- THEN result MUST contain Go source text and exit with `pass`

#### Scenario: search_content finds exported symbol

- GIVEN go-cobra fixture is present
- WHEN `search_content(query="type Command struct")` is called
- THEN at least one match MUST be returned from `command.go`

#### Scenario: extract_symbols returns package-level declarations

- GIVEN go-cobra fixture is present
- WHEN `extract_symbols(path="cobra.go")` is called
- THEN result MUST include ≥ 3 symbols of kind `function` or `type`

#### Scenario: safe_refactor rename on exported func (preview only)

- GIVEN go-cobra fixture is present
- WHEN `safe_refactor(action="rename", target="Execute", preview_only=true)` is called
- THEN result MUST return `pass` or `capability_missing` (NOT `error`)

---

## Correctness Metrics

| Metric                         | Target    |
|--------------------------------|-----------|
| Manifest schema validation     | 100% pass |
| `go build ./...` success       | 100%      |
| `go test ./...` success        | 100%      |
| Analysis pipeline errors       | 0         |
| Fixture size on disk           | ≤ 5 MB    |
