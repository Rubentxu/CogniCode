//! The shared taint detector (one definition for every weakness) and its
//! declared reach.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::catalog::Weakness;
use crate::domain::findings::{
    AnalysisCapability, DetectorAuthority, DetectorFindingPolicy, DetectorId, DetectorIr,
    DetectorStep, FindingKind, FindingSeverity, RiskLevel, SubjectPattern,
};

/// Subject carried by statements that read external data.
pub const USER_INPUT_SUBJECT: &str = "security.user_input";
/// Subject carried by statements that execute SQL text.
pub const SQL_EXECUTION_SUBJECT: &str = "security.sql_execution";
/// Subject carried by statements that run an OS command.
pub const COMMAND_EXECUTION_SUBJECT: &str = "security.command_execution";
/// Subject carried by statements that open or write a file by path.
pub const FILE_ACCESS_SUBJECT: &str = "security.file_access";
/// Subject carried by statements that neutralise tainted data.
pub const SANITIZER_SUBJECT: &str = "security.sanitizer";
/// The finding kind the CWE-89 rule produces.
pub const SQL_INJECTION_KIND: &str = "security.sql_injection";
/// The finding kind the CWE-78 rule produces.
pub const COMMAND_INJECTION_KIND: &str = "security.command_injection";
/// The finding kind the CWE-22 rule produces.
pub const PATH_TRAVERSAL_KIND: &str = "security.path_traversal";

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

/// The taint detector of `weakness` for `reach`.
///
/// One definition serves every language and every weakness: it mentions only
/// subjects, never an API. Language knowledge lives in the adapters that
/// attach those subjects; the weakness only chooses the sink subject and the
/// finding kind.
pub fn taint_detector(weakness: Weakness, reach: AnalysisReach) -> DetectorIr {
    let kind = weakness.finding_kind();
    let (id, name) = match reach {
        AnalysisReach::Intraprocedural => (
            kind.to_string(),
            format!("{} ({}), intraprocedural", kind, weakness.cwe()),
        ),
        AnalysisReach::Interprocedural => (
            format!("{kind}_interprocedural"),
            format!("{} ({}), interprocedural", kind, weakness.cwe()),
        ),
    };
    DetectorIr {
        id: DetectorId::new(id).expect("derived detector id is valid"),
        name,
        policy: DetectorFindingPolicy::new(FindingSeverity::Critical, RiskLevel::High),
        requires: reach.required_capabilities(),
        authority: DetectorAuthority::Candidate,
        steps: vec![
            DetectorStep::Match {
                subject: subject(USER_INPUT_SUBJECT),
            },
            DetectorStep::Flow {
                source: subject(USER_INPUT_SUBJECT),
                sink: subject(weakness.sink_subject()),
                max_hops: None,
            },
            DetectorStep::Exclude {
                path_contains: subject(SANITIZER_SUBJECT),
            },
            DetectorStep::Produce {
                kind: FindingKind::new(kind).expect("constant kind is valid"),
            },
        ],
    }
}

/// The CWE-89 detector for `reach`.
pub fn cwe89_sql_injection(reach: AnalysisReach) -> DetectorIr {
    taint_detector(Weakness::SqlInjection, reach)
}

fn subject(name: &str) -> SubjectPattern {
    SubjectPattern::new(name).expect("constant subject names are valid namespaced names")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_are_valid_namespaced_names() {
        for name in [
            USER_INPUT_SUBJECT,
            SQL_EXECUTION_SUBJECT,
            COMMAND_EXECUTION_SUBJECT,
            FILE_ACCESS_SUBJECT,
            SANITIZER_SUBJECT,
        ] {
            assert!(SubjectPattern::new(name).is_ok(), "{name}");
        }
        for kind in [
            SQL_INJECTION_KIND,
            COMMAND_INJECTION_KIND,
            PATH_TRAVERSAL_KIND,
        ] {
            assert!(FindingKind::new(kind).is_ok(), "{kind}");
        }
    }

    #[test]
    fn both_reaches_validate_and_declare_their_exact_capabilities() {
        for reach in [
            AnalysisReach::Intraprocedural,
            AnalysisReach::Interprocedural,
        ] {
            for weakness in Weakness::ALL {
                let ir = taint_detector(weakness, reach);
                assert_eq!(ir.validate(), Ok(()), "{weakness:?} {reach:?}");
                assert_eq!(ir.requires, reach.required_capabilities());
            }
        }
    }

    #[test]
    fn each_weakness_has_its_own_detector_identity_and_sink() {
        let digests: BTreeSet<_> = Weakness::ALL
            .iter()
            .map(|w| taint_detector(*w, AnalysisReach::Intraprocedural).logic_digest())
            .collect();
        assert_eq!(digests.len(), Weakness::ALL.len());
        for weakness in Weakness::ALL {
            let ir = taint_detector(weakness, AnalysisReach::Intraprocedural);
            assert!(ir.steps.iter().any(|s| matches!(
                s,
                DetectorStep::Flow { sink, .. } if sink.as_str() == weakness.sink_subject()
            )));
        }
    }

    #[test]
    fn the_sql_wrapper_is_the_sql_detector() {
        assert_eq!(
            cwe89_sql_injection(AnalysisReach::Intraprocedural).logic_digest(),
            taint_detector(Weakness::SqlInjection, AnalysisReach::Intraprocedural).logic_digest()
        );
    }

    #[test]
    fn reaches_have_distinct_logic_identity() {
        assert_ne!(
            cwe89_sql_injection(AnalysisReach::Intraprocedural).logic_digest(),
            cwe89_sql_injection(AnalysisReach::Interprocedural).logic_digest()
        );
    }
}
