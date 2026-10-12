//! CogniCode Core - Code intelligence library for AI agents

// B4.W2 — architecture as diagnostic (read-only surface, no gate authority).
pub mod architecture;
// B3.W4 — análisis incremental fiable (BLAKE3 + CacheStore por sesión).
pub mod application;
pub mod assurance_export;
pub mod domain;
// B4.W3 — normalized quality finding representation across the 5
// quality tools (complexity, long_parameter_lists, god_functions,
// solid_audit, relationship_cycle). Pure data; no I/O.
pub mod findings;
// B4.W5 — Composite impact query for agents (evidence, not verdict).
pub mod impact;
pub mod incremental;
pub mod infrastructure;
pub mod interface;
pub mod product;
// B5 — Safe refactoring & authorized changes (v0.101.15).
//   B5.W1: `refactor::plan` (id, workspace_identity, basis,
//          edits, preconditions, risks + sha256 digest).
//   B5.W3: `refactor::apply` (autoridad separada, transactional
//          writes, rollback, AAT-B5-01..03).
pub mod refactor;
pub mod sandbox_core;
pub mod schemas;

// Re-export the main facade
pub use application::dto;
pub use application::workspace_session::{WorkspaceError, WorkspaceResult, WorkspaceSession};

// Re-export CLI types
pub use interface::cli::{Cli, CommandExecutor};

// Re-export commonly used types
pub use anyhow::Context;
pub use anyhow::Result;
