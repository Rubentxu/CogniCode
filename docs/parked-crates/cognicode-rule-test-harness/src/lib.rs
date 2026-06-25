//! Rule test harness for cognicode-axiom
//!
//! Loads fixture directories with intentional code smells,
//! runs rules against them, and compares results against expected.json.

pub mod fixture;
pub mod runner;
pub mod report;

pub use fixture::{Fixture, TestCase, ExpectedIssue};
pub use runner::RuleRunner;
pub use report::TestReport;
