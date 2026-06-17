//! CogniCode MCP server library — public surface for tests and shared
//! authentication primitives.
//!
//! This library target exposes the M3.5 Bearer-token auth helper so
//! unit tests can run against it without compiling the full
//! `cognicode-mcp-server` binary. The binary targets (`cognicode-mcp`,
//! `cognicode-mcp-server`, `mcp-client`) live alongside this library
//! in `src/main.rs`, `src/server.rs`, and `src/mcp_client.rs`.
//!
//! Public modules:
//!
//! - [`auth`] — M3.5 Bearer-token comparison (`check_bearer_token`).

pub mod auth;
