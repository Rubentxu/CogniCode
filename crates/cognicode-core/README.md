# cognicode-core

Code intelligence library for AI agents: multi-language parsing, symbol and call graphs, and analysis services. It is the engine behind the `cognicode` CLI and the `cognicode-mcp` server.

## Features

`default` enables `persistence`. Every other feature is off by default.

| Feature | Enables |
|---|---|
| `persistence` | Durable snapshots and content hashing (default). |
| `multimodal` | The generic graph layer and the docs-source adapter. |
| `ownership` | CODEOWNERS parsing and git-blame authorship; calls the `git` CLI. |
| `evidence-kernel` | The evidence kernel types, ports and in-memory stores. |
| `program-analysis-server` | Server-side data-flow, interprocedural summaries and taint. |
| `contract-store` | The contract generate and validate tools; no store adapter ships yet. |
| `test-support` | Test utilities such as `MockClock`; not covered by the policy. |
| `evidence-cli-ladybug` | The `evidence` CLI subcommands bound to the LadybugDB store; for the workspace binaries. |

## Compatibility

See the [semver and API compatibility policy](https://github.com/Rubentxu/CogniCode/blob/main/docs/distribution/SEMVER_POLICY.md).

Part of [CogniCode](https://github.com/Rubentxu/CogniCode). Licensed under MIT OR Apache-2.0.
