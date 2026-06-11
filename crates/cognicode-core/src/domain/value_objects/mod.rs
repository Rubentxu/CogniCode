//! Value Objects for CogniCode Domain
//!
//! Value objects are immutable types that are defined by their attributes rather than a unique identity.

pub mod dependency_type;
pub mod edge_kind;
pub mod edge_metadata;
pub mod file_manifest;
pub mod location;
pub mod node_kind;
pub mod provenance;
pub mod source_range;
pub mod symbol_kind;

// Multimodal (brain-federation) — Space domain value objects.
// Gated behind the `multimodal` Cargo feature so the default build
// is byte-for-byte unchanged. With the feature disabled, no `space`
// symbol is exported and the PG `spaces` migration is absent.
#[cfg(feature = "multimodal")]
pub mod space;
#[cfg(feature = "multimodal")]
pub mod space_id;
#[cfg(feature = "multimodal")]
pub mod issue_properties;

pub use dependency_type::DependencyType;
pub use edge_kind::EdgeKind;
pub use edge_metadata::EdgeMetadata;
pub use file_manifest::{FileEntry, FileManifest};
pub use location::Location;
pub use node_kind::NodeKind;
pub use provenance::Provenance;
pub use source_range::SourceRange;
pub use symbol_kind::SymbolKind;

// Multimodal re-exports — Space + SpaceId live in `value_objects::space`
// (with `SpaceId` re-exported at the top-level for ergonomic
// `use cognicode_core::domain::value_objects::SpaceId;`).
#[cfg(feature = "multimodal")]
pub use space::{Space, SpaceError, SpaceKind};
#[cfg(feature = "multimodal")]
pub use space_id::SpaceId;
