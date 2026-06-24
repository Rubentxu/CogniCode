//! Schemas module — shared data types for MCP tools.
//!
//! Re-exports built-in view descriptors from [`builtin_descriptors`].
//! The 8 built-in descriptors are the single source of truth shared
//! between MCP handlers and the explorer registry.

pub mod builtin_descriptors;

pub use builtin_descriptors::{BUILTIN_DESCRIPTORS_RAW, BuiltinDescriptorRaw, builtin_descriptors};
