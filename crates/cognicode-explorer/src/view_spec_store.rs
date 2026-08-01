//! Relocated to `cognicode_core::domain::ports::view_spec_store`.
//! This re-export shim preserves the import path for consumers that
//! still resolve `cognicode_explorer::registry::ViewSpecStore`.
pub use cognicode_core::domain::ports::view_spec_store::{
    PostgresViewSpecStore, ViewSpecPayload, ViewSpecStore, ViewSpecStoreError,
};
