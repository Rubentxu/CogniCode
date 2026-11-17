//! The shared vocabulary of the port conformance suites.
//!
//! A conformance suite is a function that drives ONE adapter through
//! the clauses of a port contract and reports every clause the adapter
//! breaks, instead of stopping at the first. The suites of
//! [`crate::domain::scm::conformance`] and
//! [`crate::domain::forge::conformance`] return a [`ConformanceReport`];
//! an adapter's own test asserts that the report is clean, and a
//! deliberately broken adapter asserts that a named clause is reported.
//! A suite that cannot fail proves nothing, so the second use is as
//! important as the first.
//!
//! Compiled for unit tests and under the `test-support` feature, so a
//! crate that adds another adapter can reuse the suites.

use std::collections::BTreeSet;
use std::fmt::Debug;

/// One clause of a port contract that an adapter did not honour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClauseViolation {
    /// Stable identifier of the clause, such as `scm.resolve.unknown`.
    pub clause: &'static str,
    /// What the adapter did instead, in terms a reader can act on.
    pub detail: String,
}

/// The clauses an adapter broke while a suite ran. Empty means the
/// adapter conforms to every clause the suite exercised.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ConformanceReport {
    violations: Vec<ClauseViolation>,
}

impl ConformanceReport {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records that `clause` was broken.
    pub fn violated(&mut self, clause: &'static str, detail: impl Into<String>) {
        self.violations.push(ClauseViolation {
            clause,
            detail: detail.into(),
        });
    }

    /// Records a violation of `clause` unless `holds`. The detail is
    /// built only when the clause is broken.
    pub fn require(&mut self, clause: &'static str, holds: bool, detail: impl FnOnce() -> String) {
        if !holds {
            self.violated(clause, detail());
        }
    }

    /// Records a violation of `clause` unless `actual` equals `expected`.
    pub fn expect_eq<T: PartialEq + Debug>(
        &mut self,
        clause: &'static str,
        what: &str,
        actual: &T,
        expected: &T,
    ) {
        self.require(clause, actual == expected, || {
            format!("{what}: expected {expected:?}, got {actual:?}")
        });
    }

    /// Adds every violation of `other` to this report.
    pub fn absorb(&mut self, other: ConformanceReport) {
        self.violations.extend(other.violations);
    }

    pub fn violations(&self) -> &[ClauseViolation] {
        &self.violations
    }

    pub fn is_clean(&self) -> bool {
        self.violations.is_empty()
    }

    /// Whether `clause` was reported as broken at least once.
    pub fn reports(&self, clause: &str) -> bool {
        self.violations.iter().any(|v| v.clause == clause)
    }

    /// The distinct clauses reported as broken.
    pub fn clauses(&self) -> BTreeSet<&'static str> {
        self.violations.iter().map(|v| v.clause).collect()
    }

    /// Panics listing every violation unless the report is clean. This
    /// is what an adapter's conformance test calls.
    #[track_caller]
    pub fn assert_conforms(&self, adapter: &str) {
        assert!(
            self.is_clean(),
            "{adapter} breaks {} clause(s):\n{}",
            self.violations.len(),
            self.violations
                .iter()
                .map(|v| format!("  [{}] {}", v.clause, v.detail))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    /// Panics unless `clause` was reported. This is what a negative
    /// control calls to prove the suite can see the flaw.
    #[track_caller]
    pub fn assert_detects(&self, flaw: &str, clause: &str) {
        assert!(
            self.reports(clause),
            "the suite did not detect `{flaw}` as a violation of [{clause}]; it reported {:?}",
            self.clauses()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_lists_every_broken_clause_instead_of_stopping_at_the_first() {
        let mut report = ConformanceReport::new();
        report.require("a.first", false, || "first".to_string());
        report.require("a.fine", true, || unreachable!("built only when broken"));
        report.expect_eq("a.second", "value", &1, &2);

        assert_eq!(
            report.clauses(),
            BTreeSet::from(["a.first", "a.second"]),
            "{report:?}"
        );
        assert!(report.reports("a.second"));
        assert!(!report.reports("a.fine"));
        assert!(!report.is_clean());
    }

    #[test]
    #[should_panic(expected = "breaks 1 clause(s)")]
    fn a_dirty_report_does_not_conform() {
        let mut report = ConformanceReport::new();
        report.violated("a.clause", "broken");
        report.assert_conforms("adapter");
    }

    #[test]
    #[should_panic(expected = "did not detect")]
    fn a_control_fails_when_its_flaw_goes_unreported() {
        ConformanceReport::new().assert_detects("a flaw", "a.clause");
    }
}
