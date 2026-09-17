# e85 WU3 — The Published Product Surface

Not every binary Cargo can build is a distribution artifact. Classification is a
decision, and it is encoded in one table
(`release_contract::COMPONENTS`) rather than in workflow YAML.

## Classification

| Binary | Crate | Class | Rationale |
|---|---|---|---|
| `cogh` | cognicode-cli | **Layer 0 — published** | The bootstrap. Required to install anything. |
| `cognicode` | cognicode-cli | **Layer 1 — published** | The daily CLI. The `core` profile's only component. |
| `cognicode-mcp` | cognicode-mcp (`src/main.rs`) | **Layer 1 — published** | The stdio MCP server: the reviewer profile's daemon. |
| `explorer-api` | cognicode-runtime (`src/bin/api.rs`) | **PUBLIC PRODUCT, not in the e85 surface** | A real product binary, but its own profile semantics are a later decision. Classified, not published, not deleted. |
| `cognicode-mcp-server` | cognicode-mcp (`src/server.rs`) | **FUTURE — container deployment** | An HTTP/SSE deployment variant ("standalone container-ready"), not a local install component. |
| `explorer-mcp` | cognicode-runtime (`src/bin/mcp.rs`) | **FUTURE** | Overlaps with `cognicode-mcp` for the local MCP role. |
| `sandbox-orchestrator` | cognicode-sandbox | **FUTURE — sandbox profile** | Belongs to a sandbox profile that e85 does not publish. |
| `mcp-client` | cognicode-mcp (`src/mcp_client.rs`) | **INTERNAL — must never be packaged** | An end-to-end test client ("E2E test client using rmcp SDK"). |

## Strong default, honoured

```text
core      -> cognicode
reviewer  -> cognicode, cognicode-mcp
```

`full` is **deliberately absent**. e84 WU3 says to omit it honestly rather than
publish entries for skills/sandbox assets that are not produced. `PUBLISHED_PROFILES`
contains only `core` and `reviewer`, and `BundleManifest::validate` rejects any
profile that resolves to zero components, so a `full` profile cannot be introduced
by accident.

Adding `full` later is additive and does not break `core` or `reviewer`.

## Why the table lives in Rust

A workflow that decides which binaries to package re-creates the classification
in YAML, where it can drift from the manifest. Instead:

```text
release_contract::COMPONENTS        -> the decision
cognicode-release plan              -> the per-lane ledger
cognicode-release name              -> the canonical filename
```

The workflow loops over `plan`'s output and never names a component, a platform
token, or a digest itself.
