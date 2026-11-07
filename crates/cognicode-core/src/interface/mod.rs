//! Interface Layer - External interfaces (CLI, MCP, LSP)

pub mod catalog;
pub mod cli;
pub mod composition;
pub mod coverage;
// TASK-92 (slice 1) — the daemon's local socket: how a client on this
// machine asks whether the index the daemon keeps is still fresh.
pub mod daemon;
// TASK-82 — el registro de goals: una declaración y sus cuatro superficies.
pub mod goals;
// TASK-83 — el sobre de la respuesta: result, basis, budget y enlaces.
pub mod hateoas;
pub mod intent;
pub mod lsp;
pub mod mcp;
// TASK-85 — las recetas: preguntas compuestas, con su memo y su presupuesto.
pub mod recipes;
pub mod symbol_identity;
// TASK-84 — la superficie que el servidor publica: los perfiles `default`,
// `full` y `legacy`.
pub mod surface;
// Lo que se ve del workspace, compartido por el doctor y el goal `status`.
pub mod workspace;

#[cfg(feature = "rig")]
pub mod rig;
