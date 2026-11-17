//! Native code-quality metrics: the vocabulary, the honest capability matrix
//! and the stable evidence a measurement leaves behind.
//!
//! This module is the single owner of *what is measured and what the numbers
//! mean*. The adapter that walks syntax trees lives in
//! `infrastructure::native_metrics`; policy consumers (quality gates) read the
//! numbers and apply thresholds, they never recompute a formula.
//!
//! ## What is measured
//!
//! | metric | unit | definition |
//! |---|---|---|
//! | [`MetricKind::Cyclomatic`] | paths | `1` + one per decision: `if`, loop, non-default `match` arm / `when` entry / `switch` case, `catch`, ternary, `&&`, `||`, elvis `?:`, Rust `?` and `let ... else` |
//! | [`MetricKind::Cognitive`] | effort | structural increments weighted by nesting: `if`, loops, `switch`/`match`/`when` counted **once** per construct, `catch`, ternary and closures nest; `else if` and `else` are flat `+1`; runs of one boolean operator count once; labelled jumps `+1` |
//! | [`MetricKind::Nesting`] | levels | deepest chain of enclosing `if`, loops, `switch`/`match`/`when`/`select` and `try` |
//! | [`MetricKind::Size`] | lines | physical lines, code lines (neither blank nor comment-only) and parameters |
//! | [`MetricKind::FanOut`] | callees | distinct syntactic callees named in the body |
//!
//! ## Semantic exceptions
//!
//! * A default arm (`_` without guard, Kotlin `else`, Java/Go `default`) adds
//!   nothing to cyclomatic complexity; every other arm adds one. A guarded
//!   wildcard is a real branch.
//! * Or-patterns and multi-constant cases (`A | B =>`, `case 1, 2 ->`,
//!   `1, 2 ->`) are one branch. Java's legacy stacked `case 1: case 2:` labels
//!   are one branch per label because each label is a distinct way in.
//! * Cognitive complexity counts a whole `match`/`when`/`switch` once, not once
//!   per arm.
//! * Nested **named** functions are measured as their own unit and excluded
//!   from the enclosing one; closures and lambdas belong to the enclosing
//!   function.
//!
//! ## Honesty
//!
//! A language without a rule table is [`MetricsOutcome::Unsupported`]: no
//! function list, no zeros. Where a metric is measured with a known limit the
//! matrix says [`MetricSupport::Partial`] and the report carries the limits, so
//! a number is never read as more than it is. A file with syntax errors is
//! reported with [`MetricsCoverage::Partial`].
//!
//! Pure domain: no I/O, no parser.

use serde::{Deserialize, Serialize};

use super::findings::digest::sha256_hex;
use super::value_objects::Language;

/// Version of the measurement rules. It travels in every report so two
/// reports are comparable only when their rules were the same.
pub const METRICS_RULES_VERSION: &str = "1";

/// A family of measurements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricKind {
    /// McCabe cyclomatic complexity.
    Cyclomatic,
    /// Cognitive complexity.
    Cognitive,
    /// Control-flow nesting depth.
    Nesting,
    /// Physical lines, code lines and parameters.
    Size,
    /// Distinct callees named in a body.
    FanOut,
}

impl MetricKind {
    /// Every metric, in the order reports list them.
    pub const ALL: [MetricKind; 5] = [
        MetricKind::Cyclomatic,
        MetricKind::Cognitive,
        MetricKind::Nesting,
        MetricKind::Size,
        MetricKind::FanOut,
    ];

    /// Stable lowercase name, the one reports and policies use.
    pub const fn as_str(self) -> &'static str {
        match self {
            MetricKind::Cyclomatic => "cyclomatic",
            MetricKind::Cognitive => "cognitive",
            MetricKind::Nesting => "nesting",
            MetricKind::Size => "size",
            MetricKind::FanOut => "fan_out",
        }
    }
}

/// How well a metric is measured for one language.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum MetricSupport {
    /// Measured by the full rule set.
    Supported,
    /// Measured, with known limits a reader must keep in mind.
    Partial {
        /// What the measurement does not capture.
        limits: Vec<String>,
    },
    /// Not measured: no rule exists for this language.
    Unsupported {
        /// Why nothing is measured.
        reason: String,
    },
}

/// One row of the capability matrix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetricCapability {
    /// The metric the row speaks about.
    pub metric: MetricKind,
    /// How well it is measured.
    pub support: MetricSupport,
}

/// Whether `language` has a native rule table.
pub fn is_measurable(language: Language) -> bool {
    matches!(
        language,
        Language::Rust | Language::Java | Language::Kotlin | Language::Go
    )
}

const COGNITIVE_LIMITS: &[&str] = &[
    "recursion is not counted as an increment",
    "the `?` operator and elvis `?:` add no cognitive increment",
];

const FAN_OUT_LIMITS: &[&str] = &[
    "syntactic: method calls are keyed by method name, not by resolved receiver type",
    "macro invocations and callee-less constructs are not counted",
];

/// The honest capability of every metric for `language`.
///
/// A language outside the supported set yields `Unsupported` for every metric,
/// never a zero.
pub fn capability_matrix(language: Language) -> Vec<MetricCapability> {
    MetricKind::ALL
        .iter()
        .map(|&metric| MetricCapability {
            metric,
            support: support_of(language, metric),
        })
        .collect()
}

fn support_of(language: Language, metric: MetricKind) -> MetricSupport {
    if !is_measurable(language) {
        return MetricSupport::Unsupported {
            reason: format!(
                "no native metric rules for {}; nothing was measured",
                language.name()
            ),
        };
    }
    let limits = match metric {
        MetricKind::Cognitive => COGNITIVE_LIMITS,
        MetricKind::FanOut => FAN_OUT_LIMITS,
        _ => return MetricSupport::Supported,
    };
    MetricSupport::Partial {
        limits: limits.iter().map(|l| (*l).to_string()).collect(),
    }
}

/// Size of one function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SizeMetrics {
    /// Physical lines of the declaration, first to last, inclusive.
    pub lines: u32,
    /// Lines that are neither blank nor comment-only.
    pub code_lines: u32,
    /// Declared parameters, receivers excluded.
    pub parameters: u32,
}

/// Every metric of one function.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionMetrics {
    /// Stable identity: `path:qualified_name:start_line`.
    pub id: String,
    /// Declared name.
    pub name: String,
    /// Enclosing type, impl, class or receiver, when there is one.
    pub container: Option<String>,
    /// First line, 1-based.
    pub start_line: u32,
    /// Last line, 1-based.
    pub end_line: u32,
    /// Cyclomatic complexity.
    pub cyclomatic: u32,
    /// Cognitive complexity.
    pub cognitive: u32,
    /// Deepest control-flow nesting.
    pub nesting_depth: u32,
    /// Size of the function.
    pub size: SizeMetrics,
    /// Number of distinct callees; equals `callees.len()`.
    pub fan_out: u32,
    /// The distinct callees, sorted: the evidence behind `fan_out`.
    pub callees: Vec<String>,
    /// The function contains syntax the grammar could not read, so its
    /// numbers may be lower than the truth.
    pub syntax_errors: bool,
}

impl FunctionMetrics {
    /// The single number a threshold compares for `metric`.
    ///
    /// `Size` answers with physical `lines`; the other parts of the size are
    /// read from [`FunctionMetrics::size`].
    pub fn value(&self, metric: MetricKind) -> u32 {
        match metric {
            MetricKind::Cyclomatic => self.cyclomatic,
            MetricKind::Cognitive => self.cognitive,
            MetricKind::Nesting => self.nesting_depth,
            MetricKind::Size => self.size.lines,
            MetricKind::FanOut => self.fan_out,
        }
    }
}

/// Size of a whole file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSize {
    /// Physical lines.
    pub lines: u32,
    /// Lines that are neither blank nor comment-only.
    pub code_lines: u32,
    /// Functions measured in the file.
    pub functions: u32,
}

/// How complete a measurement is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum MetricsCoverage {
    /// The whole file was read without syntax errors.
    Complete,
    /// Something could not be read; numbers are lower bounds where it matters.
    Partial {
        /// What was not covered.
        reasons: Vec<String>,
    },
}

/// The measurements of one file with everything needed to reproduce them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMetrics {
    /// Path as given by the caller.
    pub path: String,
    /// Lowercase language name.
    pub language: String,
    /// Version of the rules that produced the numbers.
    pub rules_version: String,
    /// SHA-256 of the measured bytes.
    pub source_digest: String,
    /// Size of the file.
    pub size: FileSize,
    /// Functions, in source order.
    pub functions: Vec<FunctionMetrics>,
    /// What the file measurement covers.
    pub coverage: MetricsCoverage,
    /// How each metric is supported for this language.
    pub capabilities: Vec<MetricCapability>,
}

impl FileMetrics {
    /// Digest of the whole report: equal inputs and rules give equal digests.
    pub fn evidence_digest(&self) -> String {
        // The report is plain data with a fixed field and element order, so
        // its JSON is canonical for a given rule version.
        let bytes = serde_json::to_vec(self).unwrap_or_default();
        sha256_hex(&bytes)
    }
}

/// The result of asking for the metrics of a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum MetricsOutcome {
    /// The file was measured.
    Measured {
        /// The measurements.
        metrics: Box<FileMetrics>,
    },
    /// The language has no native rules; nothing was measured.
    Unsupported {
        /// Lowercase language name.
        language: String,
        /// Why nothing was measured.
        reason: String,
    },
    /// Measurement was attempted and failed; no figure is made up.
    Failed {
        /// What went wrong.
        reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_supported_language_reports_every_metric_and_names_known_limits() {
        let matrix = capability_matrix(Language::Kotlin);
        assert_eq!(matrix.len(), MetricKind::ALL.len());
        let of = |m: MetricKind| &matrix.iter().find(|c| c.metric == m).unwrap().support;
        assert_eq!(of(MetricKind::Cyclomatic), &MetricSupport::Supported);
        assert_eq!(of(MetricKind::Nesting), &MetricSupport::Supported);
        assert_eq!(of(MetricKind::Size), &MetricSupport::Supported);
        assert!(
            matches!(of(MetricKind::Cognitive), MetricSupport::Partial { limits } if !limits.is_empty())
        );
        assert!(
            matches!(of(MetricKind::FanOut), MetricSupport::Partial { limits } if !limits.is_empty())
        );
    }

    #[test]
    fn a_language_without_rules_is_unsupported_for_every_metric_never_zero() {
        for language in [Language::Python, Language::JavaScript, Language::Ruby] {
            assert!(!is_measurable(language));
            for cap in capability_matrix(language) {
                assert!(
                    matches!(cap.support, MetricSupport::Unsupported { .. }),
                    "{}: {} must be unsupported",
                    language.name(),
                    cap.metric.as_str()
                );
            }
        }
    }

    #[test]
    fn the_four_native_languages_are_exactly_the_measurable_ones() {
        let measurable: Vec<_> = Language::all_languages()
            .iter()
            .copied()
            .filter(|l| is_measurable(*l))
            .map(|l| l.name())
            .collect();
        assert_eq!(measurable.len(), 4, "{measurable:?}");
    }

    fn sample() -> FunctionMetrics {
        FunctionMetrics {
            id: "a.rs:f:1".into(),
            name: "f".into(),
            container: None,
            start_line: 1,
            end_line: 4,
            cyclomatic: 3,
            cognitive: 5,
            nesting_depth: 2,
            size: SizeMetrics {
                lines: 4,
                code_lines: 3,
                parameters: 1,
            },
            fan_out: 2,
            callees: vec!["a".into(), "b".into()],
            syntax_errors: false,
        }
    }

    #[test]
    fn a_threshold_reads_the_value_of_each_metric_from_the_report() {
        let f = sample();
        let got: Vec<u32> = MetricKind::ALL.iter().map(|m| f.value(*m)).collect();
        assert_eq!(got, vec![3, 5, 2, 4, 2]);
    }

    #[test]
    fn the_evidence_digest_is_stable_and_moves_with_the_content() {
        let report = |cyclomatic| FileMetrics {
            path: "a.rs".into(),
            language: "rust".into(),
            rules_version: METRICS_RULES_VERSION.into(),
            source_digest: sha256_hex(b"fn f() {}"),
            size: FileSize {
                lines: 1,
                code_lines: 1,
                functions: 1,
            },
            functions: vec![FunctionMetrics {
                cyclomatic,
                ..sample()
            }],
            coverage: MetricsCoverage::Complete,
            capabilities: capability_matrix(Language::Rust),
        };
        assert_eq!(report(3).evidence_digest(), report(3).evidence_digest());
        assert_ne!(report(3).evidence_digest(), report(4).evidence_digest());
    }
}
