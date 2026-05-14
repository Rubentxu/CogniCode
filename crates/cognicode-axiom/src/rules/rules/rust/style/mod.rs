//! Style-related rules
//!
//! Rules that detect code style issues

pub mod s3491_trailing_whitespace;
pub mod style_inconsistent_naming;
pub mod style_magic_numbers;
pub mod style_short_names;

pub use s3491_trailing_whitespace::S3491Rule;
pub use style_inconsistent_naming::STYLE_002Rule;
pub use style_magic_numbers::STYLE_003Rule;
pub use style_short_names::STYLE_001Rule;