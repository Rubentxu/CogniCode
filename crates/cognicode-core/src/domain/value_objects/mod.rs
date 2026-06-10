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

pub use dependency_type::DependencyType;
pub use edge_kind::EdgeKind;
pub use edge_metadata::EdgeMetadata;
pub use file_manifest::{FileEntry, FileManifest};
pub use location::Location;
pub use node_kind::NodeKind;
pub use provenance::Provenance;
pub use source_range::SourceRange;
pub use symbol_kind::SymbolKind;
