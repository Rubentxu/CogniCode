//! The shared CWE-89 detector and its declared reach.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::domain::findings::{
    AnalysisCapability, DetectorAuthority, DetectorFindingPolicy, DetectorId, DetectorIr,
    DetectorStep, FindingKind, SubjectPattern,
};

/// Subject carried by statements that read external data.
pub const USER_INPUT_SUBJECT: &str = "security.user_input";
/// Subject carried by statements that execute SQL text.
pub const SQL_EXECUTION_SUBJECT: &str = "security.sql_execution";
/// Subject carried by statements that neutralise tainted data.
pub const SANITIZER_SUBJECT: &str = "security.sanitizer";
/// The finding kind the CWE-89 rule produces.
pub const SQL_INJECTION_KIND: &str = "security.sql_injection";

/// How far a flow may travel: the explicit intra/interprocedural limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisReach {
    /// Within one function. Provided today by the M5 dataflow backend.
    Intraprocedural,
    /// Across calls. Needs callee resolution and a call graph on top of
    /// dataflow, so it is only runnable when a backend provides all three.
    Interprocedural,
}

impl AnalysisReach {
    /// The exact capabilities a rule of this reach needs.
    pub fn required_capabilities(self) -> BTreeSet<AnalysisCapability> {
        match self {
            Self::Intraprocedural => [AnalysisCapability::Dataflow].into_iter().collect(),
            Self::Interprocedural => [
                AnalysisCapability::Dataflow,
                AnalysisCapability::SemanticResolution,
                AnalysisCapability::GraphQuery,
            ]
            .into_iter()
            .collect(),
        }
    }

    /// Stable name for diagnostics.
    pub fn name(self) -> &'static str {
        match self {
            Self::Intraprocedural => "intraprocedural",
            Self::Interprocedural => "interprocedural",
        }
    }
}

/// The CWE-89 detector for `reach`.
///
/// One definition serves every language: it mentions only subjects, never an
/// API. Language knowledge lives in the adapters that attach those subjects.
pub fn cwe89_sql_injection(reach: AnalysisReach) -> DetectorIr {
    let (id, name) = match reach {
        AnalysisReach::Intraprocedural => (
            "security.sql_injection",
            "SQL injection (CWE-89), intraprocedural",
        ),
        AnalysisReach::Interprocedural => (
            "security.sql_injection_interprocedural",
            "SQL injection (CWE-89), interprocedural",
        ),
    };
    DetectorIr {
        id: DetectorId::new(id).expect("constant detector id is valid"),
        name: name.to_string(),
        policy: DetectorFindingPolicy::default(),
        requires: reach.required_capabilities(),
        authority: DetectorAuthority::Candidate,
        steps: vec![
            DetectorStep::Match {
                subject: subject(USER_INPUT_SUBJECT),
            },
            DetectorStep::Flow {
                source: subject(USER_INPUT_SUBJECT),
                sink: subject(SQL_EXECUTION_SUBJECT),
                max_hops: None,
            },
            DetectorStep::Exclude {
                path_contains: subject(SANITIZER_SUBJECT),
            },
            DetectorStep::Produce {
                kind: FindingKind::new(SQL_INJECTION_KIND).expect("constant kind is valid"),
            },
        ],
    }
}

fn subject(name: &str) -> SubjectPattern {
    SubjectPattern::new(name).expect("constant subject names are valid namespaced names")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_are_valid_namespaced_names() {
        for name in [USER_INPUT_SUBJECT, SQL_EXECUTION_SUBJECT, SANITIZER_SUBJECT] {
            assert!(SubjectPattern::new(name).is_ok(), "{name}");
        }
        assert!(FindingKind::new(SQL_INJECTION_KIND).is_ok());
    }

    #[test]
    fn both_reaches_validate_and_declare_their_exact_capabilities() {
        for reach in [
            AnalysisReach::Intraprocedural,
            AnalysisReach::Interprocedural,
        ] {
            let ir = cwe89_sql_injection(reach);
            assert_eq!(ir.validate(), Ok(()), "{reach:?}");
            assert_eq!(ir.requires, reach.required_capabilities(), "{reach:?}");
        }
    }

    #[test]
    fn reaches_have_distinct_logic_identity() {
        assert_ne!(
            cwe89_sql_injection(AnalysisReach::Intraprocedural).logic_digest(),
            cwe89_sql_injection(AnalysisReach::Interprocedural).logic_digest()
        );
    }
}
