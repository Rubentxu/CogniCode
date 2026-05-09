//! C4 Model types — agnostic representation for all diagram levels

pub mod c4_types;
pub mod relationships;
pub mod workspace;
pub mod views;
pub mod styles;

pub use c4_types::*;
pub use relationships::*;
pub use workspace::C4Workspace;
