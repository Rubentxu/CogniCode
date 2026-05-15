//! Security Input Validation Rules Module
//!
//! Rules for detecting security issues in input handling: SQL injection,
//! command injection, path traversal, XSS, and other injection vulnerabilities.

pub mod cc_sec_inp_001;
pub mod cc_sec_inp_002;
pub mod cc_sec_inp_003;
pub mod cc_sec_inp_004;
pub mod cc_sec_inp_005;
pub mod cc_sec_inp_006;
pub mod cc_sec_inp_007;
pub mod cc_sec_inp_008;
pub mod cc_sec_inp_009;
pub mod cc_sec_inp_010;
pub mod cc_sec_inp_011;
pub mod cc_sec_inp_012;
pub mod cc_sec_inp_013;
pub mod cc_sec_inp_014;

// Re-export all security input validation rules
pub use cc_sec_inp_001::SqlInjectionRule;
pub use cc_sec_inp_002::CommandInjectionRule;
pub use cc_sec_inp_003::PathTraversalRule;
pub use cc_sec_inp_004::XxeInjectionRule;
pub use cc_sec_inp_005::InsecureDeserializationRule;
pub use cc_sec_inp_006::LdapInjectionRule;
pub use cc_sec_inp_007::XpathInjectionRule;
pub use cc_sec_inp_008::CrossSiteScriptingRule;
pub use cc_sec_inp_009::PathEquivalenceRule;
pub use cc_sec_inp_010::OpenRedirectRule;
pub use cc_sec_inp_011::HttpResponseSplittingRule;
pub use cc_sec_inp_012::IntegerOverflowRule;
pub use cc_sec_inp_013::MissingInputSanitizationRule;
pub use cc_sec_inp_014::UnvalidatedUrlSchemeRule;