//! MCP Interface - Model Context Protocol implementation
//!
//! The server announces the capabilities it serves and no others. It
//! serves `tools`; it does not serve `prompts`, `resources` or
//! `completion`, so it does not announce them, and the modules that
//! implemented them without ever being called are gone rather than
//! waiting for a caller that never came.

pub mod dto_mapping;
pub mod error;
pub mod file_ops_handlers;
pub mod handlers;
pub mod schemas;
pub mod security;
// El presupuesto del trabajo de una herramienta, para la ventana del despacho.
pub mod work_budget;

pub use error::{InterfaceError, InterfaceResult};
