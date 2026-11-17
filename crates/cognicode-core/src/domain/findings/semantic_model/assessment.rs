//! Certainty: what a run may honestly say, and how that is measured.

use serde::{Deserialize, Serialize};

use super::catalog::SourceLanguage;
use super::classify::Classification;
use super::rule::AnalysisReach;
use crate::domain::findings::DetectorDiagnostic;

/// What a semantic run may claim.
///
/// There is deliberately no "clean" variant: the strongest negative statement
/// is [`SastVerdict::NoFlowObserved`], which names the reach it covers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SastVerdict {
    /// The engine reported `findings` flows from external data to the sink.
    Vulnerable {
        /// Number of findings.
        findings: usize,
    },
    /// Every call was modeled and no flow was found, within `reach` only.
    NoFlowObserved {
        /// The reach the statement is limited to.
        reach: AnalysisReach,
    },
    /// Nothing was found but calls remain unmodeled: a flow may hide behind
    /// them.
    Unknown {
        /// Callees the adapter does not know, sorted.
        unresolved: Vec<String>,
    },
    /// The language has no adapter: nothing was analysed.
    Unsupported {
        /// The language without coverage.
        language: SourceLanguage,
    },
}

/// A verdict plus the certainty diagnostics that qualify it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assessment {
    /// What the run may claim.
    pub verdict: SastVerdict,
    /// Stable-coded notes on the limits of that claim.
    pub diagnostics: Vec<DetectorDiagnostic>,
}

fn diagnostic(code: &str, message: String) -> DetectorDiagnostic {
    DetectorDiagnostic {
        code: code.to_string(),
        message,
    }
}

/// Judge a run: `findings` is the number of findings the shared detector
/// produced over the classified function (zero when nothing was analysed).
///
/// The shipped backend is intraprocedural, so every negative statement is
/// scoped to [`AnalysisReach::Intraprocedural`].
pub fn assess(classification: &Classification, findings: usize) -> Assessment {
    match classification {
        Classification::Unsupported { language } => Assessment {
            verdict: SastVerdict::Unsupported {
                language: *language,
            },
            diagnostics: vec![diagnostic(
                "sast.language_unsupported",
                format!("no API model adapter for {language:?}: the function was not analysed"),
            )],
        },
        Classification::Modeled(modeled) => {
            let unresolved = modeled.unmodeled_callees();
            match (findings, unresolved.is_empty()) {
                (0, true) => Assessment {
                    verdict: SastVerdict::NoFlowObserved {
                        reach: AnalysisReach::Intraprocedural,
                    },
                    diagnostics: vec![diagnostic(
                        "sast.reach_intraprocedural",
                        "no flow found within the function; calls into other functions \
                         were not followed"
                            .to_string(),
                    )],
                },
                (0, false) => Assessment {
                    verdict: SastVerdict::Unknown {
                        unresolved: unresolved.to_vec(),
                    },
                    diagnostics: vec![diagnostic(
                        "sast.unresolved_callee",
                        format!(
                            "no flow found, but {} could not be resolved: a source or sink \
                             may sit behind them",
                            unresolved.join(", ")
                        ),
                    )],
                },
                (n, true) => Assessment {
                    verdict: SastVerdict::Vulnerable { findings: n },
                    diagnostics: vec![],
                },
                (n, false) => Assessment {
                    verdict: SastVerdict::Vulnerable { findings: n },
                    diagnostics: vec![diagnostic(
                        "sast.flow_crosses_unresolved_callee",
                        format!(
                            "taint propagates through {} unchanged; if one of them sanitizes, \
                             the finding is a false positive",
                            unresolved.join(", ")
                        ),
                    )],
                },
            }
        }
    }
}

/// A confusion tally over a labeled corpus.
///
/// `Unknown` and `Unsupported` are not decisions, so they are counted apart
/// from negatives: an undecided vulnerable case lowers recall, an undecided
/// safe case lowers neither precision nor recall but stays visible.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PrecisionRecall {
    tp: usize,
    fp: usize,
    tn: usize,
    fn_: usize,
    unknown_vulnerable: usize,
    unknown_safe: usize,
}

impl PrecisionRecall {
    /// Record one case: `vulnerable` is the ground-truth label.
    pub fn record(&mut self, vulnerable: bool, verdict: &SastVerdict) {
        match (vulnerable, verdict) {
            (true, SastVerdict::Vulnerable { .. }) => self.tp += 1,
            (false, SastVerdict::Vulnerable { .. }) => self.fp += 1,
            (true, SastVerdict::NoFlowObserved { .. }) => self.fn_ += 1,
            (false, SastVerdict::NoFlowObserved { .. }) => self.tn += 1,
            (true, SastVerdict::Unknown { .. } | SastVerdict::Unsupported { .. }) => {
                self.unknown_vulnerable += 1;
            }
            (false, SastVerdict::Unknown { .. } | SastVerdict::Unsupported { .. }) => {
                self.unknown_safe += 1;
            }
        }
    }

    /// Vulnerable cases reported as vulnerable.
    pub fn true_positives(&self) -> usize {
        self.tp
    }

    /// Safe cases reported as vulnerable.
    pub fn false_positives(&self) -> usize {
        self.fp
    }

    /// Safe cases with no flow observed.
    pub fn true_negatives(&self) -> usize {
        self.tn
    }

    /// Vulnerable cases with no flow observed.
    pub fn false_negatives(&self) -> usize {
        self.fn_
    }

    /// Vulnerable cases the model could not decide.
    pub fn unknown_on_vulnerable(&self) -> usize {
        self.unknown_vulnerable
    }

    /// Safe cases the model could not decide.
    pub fn unknown_on_safe(&self) -> usize {
        self.unknown_safe
    }

    /// `tp / (tp + fp)`, or `None` when nothing was reported.
    pub fn precision(&self) -> Option<f64> {
        ratio(self.tp, self.tp + self.fp)
    }

    /// `tp / (vulnerable cases)`, undecided ones included, or `None` when the
    /// corpus has no vulnerable case.
    pub fn recall(&self) -> Option<f64> {
        ratio(self.tp, self.tp + self.fn_ + self.unknown_vulnerable)
    }
}

fn ratio(numerator: usize, denominator: usize) -> Option<f64> {
    (denominator > 0).then(|| numerator as f64 / denominator as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::findings::semantic_model::{
        CallArg, Observation, ObservationKind, ObservedCall, ObservedFunction,
        SemanticModelCatalog, Weakness,
    };

    fn classify(callees: &[&str], language: SourceLanguage) -> Classification {
        SemanticModelCatalog::builtin().classify(
            Weakness::SqlInjection,
            &ObservedFunction {
                id: "f".to_string(),
                path: "f".to_string(),
                language,
                observations: callees
                    .iter()
                    .map(|c| {
                        Observation::new(
                            1,
                            ObservationKind::Call(ObservedCall {
                                callee: c.to_string(),
                                args: vec![CallArg::Literal],
                                def: None,
                            }),
                        )
                    })
                    .collect(),
            },
        )
    }

    #[test]
    fn nothing_found_with_every_call_modeled_is_scoped_not_clean() {
        let a = assess(&classify(&["strconv.Atoi"], SourceLanguage::Go), 0);
        assert_eq!(
            a.verdict,
            SastVerdict::NoFlowObserved {
                reach: AnalysisReach::Intraprocedural
            }
        );
        assert_eq!(a.diagnostics[0].code, "sast.reach_intraprocedural");
    }

    #[test]
    fn nothing_found_with_an_unmodeled_call_is_unknown() {
        let a = assess(&classify(&["app.helper"], SourceLanguage::Go), 0);
        assert_eq!(
            a.verdict,
            SastVerdict::Unknown {
                unresolved: vec!["app.helper".to_string()]
            }
        );
        assert_eq!(a.diagnostics[0].code, "sast.unresolved_callee");
    }

    #[test]
    fn a_finding_across_an_unmodeled_call_is_flagged_as_possibly_false() {
        let a = assess(&classify(&["app.helper"], SourceLanguage::Go), 2);
        assert_eq!(a.verdict, SastVerdict::Vulnerable { findings: 2 });
        assert_eq!(a.diagnostics[0].code, "sast.flow_crosses_unresolved_callee");
    }

    #[test]
    fn a_finding_with_every_call_modeled_needs_no_caveat() {
        let a = assess(&classify(&["strconv.Atoi"], SourceLanguage::Go), 1);
        assert_eq!(a.verdict, SastVerdict::Vulnerable { findings: 1 });
        assert!(a.diagnostics.is_empty());
    }

    #[test]
    fn unsupported_language_is_never_negative() {
        let a = assess(&classify(&["x"], SourceLanguage::Kotlin), 0);
        assert_eq!(
            a.verdict,
            SastVerdict::Unsupported {
                language: SourceLanguage::Kotlin
            }
        );
        assert_eq!(a.diagnostics[0].code, "sast.language_unsupported");
    }

    #[test]
    fn undecided_cases_lower_recall_but_not_precision() {
        let vulnerable = SastVerdict::Vulnerable { findings: 1 };
        let none = SastVerdict::NoFlowObserved {
            reach: AnalysisReach::Intraprocedural,
        };
        let unknown = SastVerdict::Unknown { unresolved: vec![] };

        let mut tally = PrecisionRecall::default();
        assert_eq!(tally.precision(), None);
        assert_eq!(tally.recall(), None);

        tally.record(true, &vulnerable);
        tally.record(true, &unknown);
        tally.record(true, &none);
        tally.record(false, &vulnerable);
        tally.record(false, &none);
        tally.record(false, &unknown);

        assert_eq!(
            (
                tally.true_positives(),
                tally.false_positives(),
                tally.true_negatives(),
                tally.false_negatives(),
                tally.unknown_on_vulnerable(),
                tally.unknown_on_safe()
            ),
            (1, 1, 1, 1, 1, 1)
        );
        assert_eq!(tally.precision(), Some(0.5));
        assert_eq!(tally.recall(), Some(1.0 / 3.0));
    }
}
