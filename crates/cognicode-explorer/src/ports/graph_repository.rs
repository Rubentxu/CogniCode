//! Re-export of the canonical `GraphRepository` port for the
//! Generic Graph Layer.
//!
//! The trait, the `SearchPage` value type, and the `GraphError`
//! domain error all live in `cognicode-core::domain::ports`. This
//! module is kept as a thin re-export so existing
//! `crate::ports::graph_repository::*` import paths continue to
//! work without breakage.
//!
//! Available in the default build. The `multimodal` feature gates
//! the WRITE/exTRACTION path only.

pub use cognicode_core::domain::ports::{GraphRepository, SearchPage};
