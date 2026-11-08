# cognicode-graph-algos

Pure graph algorithms (PageRank, personalized PageRank, god nodes, communities, dominators, slicing, taint and more) with no I/O. Compatible with WebAssembly: the optional `petgraph-adapter` feature is off by default.

## Features

No feature is on by default, so the default build compiles to `wasm32-unknown-unknown`.

| Feature | Enables |
|---|---|
| `petgraph-adapter` | Builds the optional `petgraph` dependency so `cognicode-core` can implement `GraphBuilder` for its own graph type; adds no public items here. |

## Compatibility

See the [semver and API compatibility policy](https://github.com/Rubentxu/CogniCode/blob/main/docs/distribution/SEMVER_POLICY.md).

Part of [CogniCode](https://github.com/Rubentxu/CogniCode). Licensed under MIT OR Apache-2.0.
