//! Value Objects for CogniCode Domain
//!
//! Value objects are immutable types that are defined by their attributes rather than a unique identity.

pub mod dependency_type;
pub mod file_manifest;
pub mod location;
pub mod source_range;
pub mod symbol_kind;

pub use dependency_type::DependencyType;
pub use file_manifest::{FileEntry, FileManifest};
pub use location::Location;
pub use source_range::SourceRange;
pub use symbol_kind::SymbolKind;