//! Standalone *example*, NOT a CogniCode production module. Run:
//!   rustc --edition=2024 --test test-harness/examples/analysis_outcome.rs -o /tmp/vdh_outcome_tests
//!   /tmp/vdh_outcome_tests
//! Translate to existing CogniCode IDs/DTOs before implementation; do not add
//! duplicate enums or change wire formats by copying this file verbatim.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Basis {
    pub workspace: String,
    pub revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnavailableReason {
    PortNotWired,
    FeatureDisabled,
    UnsupportedQuery,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Missing {
    pub reason: String,
    pub subjects: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalysisOutcome<T, E> {
    Complete { value: T, basis: Basis },
    Partial { value: T, basis: Basis, missing: Missing },
    Unavailable { reason: UnavailableReason },
    Failed { error: E },
}

impl<T, E> AnalysisOutcome<T, E> {
    /// Law: map(identity) == identity. Failed/Unavailable never call f.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> AnalysisOutcome<U, E> {
        match self {
            Self::Complete { value, basis } =>
                AnalysisOutcome::Complete { value: f(value), basis },
            Self::Partial { value, basis, missing } =>
                AnalysisOutcome::Partial { value: f(value), basis, missing },
            Self::Unavailable { reason } => AnalysisOutcome::Unavailable { reason },
            Self::Failed { error } => AnalysisOutcome::Failed { error },
        }
    }
}

/// Pure classification from an *already read* result. I/O and errors are
/// handled outside this function. An empty collection is known empty only if
/// it came from a successful store read with a concrete revision.
pub fn classify_quality(issues: &[String]) -> &'static str {
    if issues.is_empty() { "clean" } else { "findings" }
}

/// Unlike `unwrap_or(0)`, this distinguishes no graph read from true zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObservedCount {
    Known(u64),
    Unavailable(UnavailableReason),
    Failed(String),
}

#[cfg(test)]
mod laws {
    use super::*;
    fn basis() -> Basis {
        Basis { workspace: "fixture".into(), revision: "rev1".into() }
    }
    #[test]
    fn mapping_identity_preserves_complete_value_and_revision() {
        let outcome: AnalysisOutcome<Vec<u8>, String> =
            AnalysisOutcome::Complete { value: vec![1,2], basis: basis() };
        assert_eq!(outcome.clone().map(|x| x), outcome);
    }
    #[test]
    fn mapping_preserves_partial_missing() {
        let outcome: AnalysisOutcome<i32, String> = AnalysisOutcome::Partial {
            value: 2, basis: basis(),
            missing: Missing { reason: "budget".into(), subjects: vec!["a.rs".into()] },
        };
        assert_eq!(outcome.clone().map(|x| x), outcome);
    }
    #[test]
    fn failed_does_not_invoke_transform() {
        let outcome: AnalysisOutcome<i32, &str> = AnalysisOutcome::Failed { error: "store error" };
        let mapped = outcome.map(|_| panic!("must never run"));
        assert_eq!(mapped, AnalysisOutcome::<i32, &str>::Failed { error: "store error" });
    }
    #[test]
    fn unavailable_does_not_invoke_transform() {
        let outcome: AnalysisOutcome<i32, &str> =
            AnalysisOutcome::Unavailable { reason: UnavailableReason::PortNotWired };
        let mapped = outcome.map(|_| panic!("must never run"));
        assert_eq!(mapped, AnalysisOutcome::<i32, &str>::Unavailable {
            reason: UnavailableReason::PortNotWired,
        });
    }
    #[test]
    fn zero_is_not_unavailable() {
        assert_ne!(ObservedCount::Known(0),
            ObservedCount::Unavailable(UnavailableReason::PortNotWired));
    }
    #[test]
    fn quality_clean_requires_successful_read() {
        assert_eq!(classify_quality(&[]), "clean");
        // Err() must be handled before calling classify_quality; callers must
        // NOT do `result.unwrap_or_default()` and pass empty vec here.
    }
}
