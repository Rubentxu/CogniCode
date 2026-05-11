//! Python Code Smell Rules
//!
//! This module contains Python-specific code smell rules for detecting maintainability issues.

pub mod s138_rule;       // Long function (>50 lines)
pub mod s134_rule;       // Deep nesting (>4 levels)
pub mod s107_rule;       // Too many parameters (>7)
pub mod s1541_rule;      // Too many branches (>10)
pub mod s3776_rule;      // Cognitive complexity (>15)
pub mod s1066_rule;      // Collapsible if
pub mod s1192_rule;      // String literal duplicates
pub mod s1135_rule;      // TODO/FIXME tags
pub mod s1134_rule;      // Deprecated API usage
pub mod s1142_rule;      // Too many returns (>5)
pub mod s1186_rule;      // Empty function
pub mod s1871_rule;      // Duplicate branches
pub mod s122_rule;       // Source file too long (>1000 lines)
pub mod s104_rule;       // Module too long
pub mod s1479_rule;      // Too many methods in class (>20)

// Batch B: 15 more Python code smell rules
pub mod s1820_rule;      // Too many fields in class (>15)
pub mod s154_rule;       // High cyclomatic complexity
pub mod s1700_rule;      // Mutable default argument
pub mod s172_rule;       // print() in library code
pub mod s100_rule;       // Function naming (snake_case)
pub mod s101_rule;       // Class naming (PascalCase)
pub mod s115_rule;       // Constant naming (UPPER_CASE)
pub mod s117_rule;       // Variable naming (snake_case)
pub mod s125_rule;       // Commented-out code
pub mod s148_rule;       // Low comment ratio
pub mod s160_rule;       // Function too complex
pub mod s1643_rule;      // String concatenation in loop
pub mod s170_rule;       // Unused import
pub mod s173_rule;       // Missing type hints on public functions
pub mod s2111_rule;      // f-string without interpolation

pub use s138_rule::PY_S138Rule;
pub use s134_rule::PY_S134Rule;
pub use s107_rule::PY_S107Rule;
pub use s1541_rule::PY_S1541Rule;
pub use s3776_rule::PY_S3776Rule;
pub use s1066_rule::PY_S1066Rule;
pub use s1192_rule::PY_S1192Rule;
pub use s1135_rule::PY_S1135Rule;
pub use s1134_rule::PY_S1134Rule;
pub use s1142_rule::PY_S1142Rule;
pub use s1186_rule::PY_S1186Rule;
pub use s1871_rule::PY_S1871Rule;
pub use s122_rule::PY_S122Rule;
pub use s104_rule::PY_S104Rule;
pub use s1479_rule::PY_S1479Rule;

// Batch B exports
pub use s1820_rule::PY_S1820Rule;
pub use s154_rule::PY_S154Rule;
pub use s1700_rule::PY_S1700Rule;
pub use s172_rule::PY_S172Rule;
pub use s100_rule::PY_S100Rule;
pub use s101_rule::PY_S101Rule;
pub use s115_rule::PY_S115Rule;
pub use s117_rule::PY_S117Rule;
pub use s125_rule::PY_S125Rule;
pub use s148_rule::PY_S148Rule;
pub use s160_rule::PY_S160Rule;
pub use s1643_rule::PY_S1643Rule;
pub use s170_rule::PY_S170Rule;
pub use s173_rule::PY_S173Rule;
pub use s2111_rule::PY_S2111Rule;
