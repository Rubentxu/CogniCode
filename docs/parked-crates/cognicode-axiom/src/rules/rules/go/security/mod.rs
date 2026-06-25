//! Go Security Rules
//!
//! This module contains Go-specific security rules for vulnerability detection.

pub mod s1_rule;       // Hardcoded secrets
pub mod s2_rule;       // SQL injection via fmt.Sprintf
pub mod s3_rule;       // exec.Command with user input
pub mod s4_rule;       // os.Chmod with dangerous permissions

pub use s1_rule::GO_S2068Rule;
pub use s2_rule::GO_S2077Rule;
pub use s3_rule::GO_S1523Rule;
pub use s4_rule::GO_S2612Rule;
