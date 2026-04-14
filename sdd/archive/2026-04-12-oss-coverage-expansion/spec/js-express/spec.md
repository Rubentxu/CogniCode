# Fixture Specification: js-express (expressjs/express)

## Purpose

Add a second real-world JavaScript repo alongside chalk, raising JS parity
to ≥2 repos. Express is a minimal Node.js web framework — heavily used in
Node.js analysis benchmarks — with CommonJS module patterns, middleware
chains, and prototype-based inheritance that differ from ES module projects.

## Fixture Metadata

| Field       | Value                                        |
|-------------|----------------------------------------------|
| Repo        | expressjs/express                            |
| Language    | JavaScript                                   |
| Tier        | B (nightly, not smoke lane)                  |
| Pin target  | Latest stable tag (e.g. v4.x or v5.x)       |
| Path        | `sandbox/repos/javascript/express/`          |
| Manifest    | `sandbox/manifests/js_repos.yaml`            |
| Size budget | ≤ 5 MB on disk (excluding `node_modules`)   |

## Requirements

### Requirement: Repo Snapshot Presence

The fixture MUST contain a pinned snapshot of expressjs/express at a tagged SHA.
At minimum: `index.js`, `lib/express.js`, `lib/router/`, `package.json` MUST exist.

#### Scenario: Core files are present after setup

- GIVEN the sandbox setup script has run for js-express
- WHEN `sandbox/repos/javascript/express/` is inspected
- THEN `index.js`, `lib/express.js`, and `package.json` MUST exist
- AND `node_modules/` MUST NOT be committed into the fixture snapshot

#### Scenario: Pinned SHA is reproducible

- GIVEN the manifest entry specifies a SHA
- WHEN the setup script clones or restores the fixture
- THEN the resulting HEAD commit MUST match the pinned SHA exactly

---

### Requirement: Manifest Entry

A valid manifest entry MUST exist for js-express in `js_repos.yaml`,
containing: `language`, `repo_url`, `pinned_sha`, `description`, and `tier`.

#### Scenario: Manifest entry validates against schema

- GIVEN `sandbox/manifests/js_repos.yaml` exists and contains the js-express entry
- WHEN the manifest schema validator runs
- THEN validation MUST pass with zero errors

---

### Requirement: Validation Stages

The js-express fixture MUST define these validation stages:

| Stage   | Command                                   | Timeout |
|---------|-------------------------------------------|---------|
| install | `npm ci --frozen-lockfile \|\| npm install` | 120s  |
| syntax  | `node --check index.js`                   | 30s     |
| test    | `npm test`                                | 120s    |

#### Scenario: npm install succeeds

- GIVEN the fixture is present and Node.js ≥18 is available
- WHEN `npm ci` or `npm install` is run in `sandbox/repos/javascript/express/`
- THEN exit code MUST be 0

#### Scenario: node --check passes on index.js

- GIVEN the fixture is present
- WHEN `node --check index.js` is run
- THEN exit code MUST be 0

#### Scenario: npm test passes

- GIVEN the fixture is present and dependencies are installed
- WHEN `npm test` is run
- THEN exit code MUST be 0 and at least 1 test MUST execute

---

### Requirement: CogniCode JavaScript Analysis Compatibility

CogniCode's pipeline MUST handle CommonJS `require()` patterns and Express's
middleware function signatures without errors.

#### Scenario: read_file on index.js

- GIVEN js-express fixture is present
- WHEN `read_file(path="index.js", mode="raw")` is called
- THEN result MUST contain JavaScript source text with `pass` outcome

#### Scenario: search_content finds require pattern

- GIVEN js-express fixture is present
- WHEN `search_content(query="require(")` is called
- THEN at least 3 matches MUST be returned

#### Scenario: extract_symbols handles CommonJS exports

- GIVEN js-express fixture is present
- WHEN `extract_symbols(path="lib/express.js")` is called
- THEN result MUST be `pass` or `capability_missing` — NEVER `error`

#### Scenario: Middleware function patterns do not cause parser errors

- GIVEN js-express has files with `function(req, res, next)` middleware signatures
- WHEN any analysis tool processes such a file
- THEN result MUST NOT be `error` or contain a stack trace

---

## Correctness Metrics

| Metric                          | Target    |
|---------------------------------|-----------|
| Manifest schema validation      | 100% pass |
| `node --check` success          | 100%      |
| `npm test` success              | 100%      |
| Analysis pipeline errors        | 0         |
| Fixture size (no node_modules)  | ≤ 5 MB    |
