//! Selective Axiom → `DetectorIr` migration (M6, cycle e61, umbrella 7.7).
//!
//! **This does not revive `cognicode-axiom`.** The crate is archived; its value
//! is a migration corpus, not its old architecture. Likewise the legacy
//! SonarQube importer only carried *catalogue metadata* (`rule_id`, `name`,
//! `severity`, `language`, `description`, tags) and emitted TODO stubs — not
//! enough to responsibly fabricate an executable detector.
//!
//! ## Boundary
//!
//! ```text
//! legacy rule → AxiomRuleReader → NormalizedLegacyRule
//!                                   │
//!                                   ▼  AxiomDetectorTranslator
//!              Translated(ImportedDetectorDefinition) | Skipped(ImportDiagnostic)
//!                                   │
//!                                   ▼  (the CALLER, not the importer)
//!                      DetectorAdmission::admit(ir, version, AdmissionSource::Imported)
//!                                   │
//!                                   ▼
//!                            ExecutionPermit (Candidate)
//! ```
//!
//! ## Cardinal rule
//!
//! ```text
//! legacy authority / quality gate / severity  ≠  execution authority
//! ```
//!
//! A legacy `BLOCKER` may influence the finding **policy** (severity/risk); it
//! can never yield [`DetectorAuthority::Gated`]. The importer has no API that
//! grants authority: it returns a `DetectorIr`, never a permit.
//!
//! ## Taxonomy (D/E never produce an approximate detector)
//!
//! | Class | Legacy semantics | Requires |
//! |-------|------------------|----------|
//! | A | a single AST subject | `{AstPattern}` |
//! | B | source → sink reachability | `{GraphQuery}` |
//! | C | source/sink/sanitizer flow | `{Dataflow}` |
//! | D | needs unsupported analysis | — `Skipped(UnsupportedAnalysis)` |
//! | E | metadata only | — `Skipped(MetadataOnly)` |
//!
//! `correct incomplete > fabricated complete`.

use serde::{Deserialize, Serialize};

use crate::domain::findings::namespaced::sanitize_segment;
use crate::domain::findings::{
    AnalysisCapability, DetectorAuthority, DetectorFindingPolicy, DetectorId, DetectorIr,
    DetectorStep, FindingKind, FindingSeverity, RiskLevel, SubjectPattern,
};

// ============================================================================
// Normalized legacy input
// ============================================================================

/// Legacy severity bucket (migration policy input only — never authority).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacySeverity {
    /// `BLOCKER`.
    Blocker,
    /// `CRITICAL`.
    Critical,
    /// `MAJOR`.
    Major,
    /// `MINOR`.
    Minor,
    /// `INFO`.
    Info,
    /// Absent / unrecognised.
    Unknown,
}

impl LegacySeverity {
    /// Stable name for provenance.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Blocker => "BLOCKER",
            Self::Critical => "CRITICAL",
            Self::Major => "MAJOR",
            Self::Minor => "MINOR",
            Self::Info => "INFO",
            Self::Unknown => "UNKNOWN",
        }
    }
}

/// The executable semantics a legacy rule actually carries, if any.
///
/// This is what separates a *translatable* rule from a catalogue entry: a
/// rule whose reader could not extract executable semantics is `None`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LegacyDetection {
    /// No executable semantics — catalogue metadata only (class E).
    None,
    /// A single AST subject (class A).
    AstPattern {
        /// Namespaced subject the rule matches.
        subject: String,
    },
    /// Source → sink reachability (class B).
    Reachability {
        /// Namespaced source subject.
        source: String,
        /// Namespaced sink subject.
        sink: String,
    },
    /// Source/sink/sanitizer flow (class C).
    TaintFlow {
        /// Namespaced source subject.
        source: String,
        /// Namespaced sink subject.
        sink: String,
        /// Namespaced subjects that neutralise the flow.
        sanitizers: Vec<String>,
    },
    /// Needs analysis M6 does not provide yet (class D).
    Unsupported {
        /// Why it cannot be translated.
        reason: String,
    },
}

/// A legacy rule in normalized form, produced by an [`AxiomRuleReader`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedLegacyRule {
    /// Origin system (e.g. `"axiom"`).
    pub system: String,
    /// Legacy rule id (e.g. `"S1226"`).
    pub rule_id: String,
    /// Human-readable name.
    pub name: String,
    /// Language the rule targets.
    pub language: String,
    /// Legacy category.
    pub category: String,
    /// Legacy severity bucket.
    pub severity: LegacySeverity,
    /// Revision of the legacy corpus this rule was read from.
    pub source_revision: String,
    /// Extracted executable semantics, if any.
    pub detection: LegacyDetection,
}

/// Provenance kept **beside** the IR (the IR stays small and semantic).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegacyRuleProvenance {
    /// Origin system.
    pub system: String,
    /// Legacy rule id.
    pub legacy_rule_id: String,
    /// Legacy language.
    pub legacy_language: String,
    /// Legacy category.
    pub legacy_category: String,
    /// Legacy severity name.
    pub legacy_severity: String,
    /// Corpus revision.
    pub source_revision: String,
}

/// A translated definition plus its legacy provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedDetectorDefinition {
    /// The translated, validated definition.
    pub ir: DetectorIr,
    /// Where it came from.
    pub legacy: LegacyRuleProvenance,
}

/// Why a legacy rule was not translated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportDiagnosticKind {
    /// The rule carries no executable semantics (class E).
    MetadataOnly,
    /// The rule needs analysis M6 does not provide (class D).
    UnsupportedAnalysis,
    /// The rule's subjects/ids cannot form a valid Detector IR.
    Unmappable,
}

/// A stable, non-fabricating skip diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportDiagnostic {
    /// Legacy rule id.
    pub rule_id: String,
    /// Diagnostic kind.
    pub kind: ImportDiagnosticKind,
    /// Human-readable reason.
    pub reason: String,
}

/// Outcome of translating one legacy rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AxiomImportResult {
    /// A valid `DetectorIr` was produced (still **not** admitted).
    Translated(ImportedDetectorDefinition),
    /// The rule was skipped with a stable diagnostic — nothing was invented.
    Skipped(ImportDiagnostic),
}

// ============================================================================
// Translation
// ============================================================================

/// Legacy severity → finding policy.
///
/// **Policy only.** This never touches authority: an imported detector is
/// always admitted as `Candidate`.
pub fn policy_for(severity: LegacySeverity) -> DetectorFindingPolicy {
    let (severity, risk) = match severity {
        LegacySeverity::Blocker => (FindingSeverity::Critical, RiskLevel::Critical),
        LegacySeverity::Critical => (FindingSeverity::Critical, RiskLevel::High),
        LegacySeverity::Major => (FindingSeverity::Warning, RiskLevel::Medium),
        LegacySeverity::Minor => (FindingSeverity::Warning, RiskLevel::Low),
        LegacySeverity::Info => (FindingSeverity::Info, RiskLevel::Low),
        LegacySeverity::Unknown => (FindingSeverity::Warning, RiskLevel::Medium),
    };
    DetectorFindingPolicy::new(severity, risk)
}

/// Deterministic finding kind for a legacy rule: `{category}.{rule_id}`.
fn kind_for(rule: &NormalizedLegacyRule) -> Option<FindingKind> {
    let namespace = sanitize_segment(&rule.category);
    let name = sanitize_segment(&rule.rule_id);
    FindingKind::new(format!("{namespace}.{name}")).ok()
}

fn detector_id_for(rule: &NormalizedLegacyRule) -> Option<DetectorId> {
    let namespace = sanitize_segment(&rule.system);
    let name = sanitize_segment(&rule.rule_id);
    DetectorId::new(format!("{namespace}.{name}")).ok()
}

fn skip(
    rule: &NormalizedLegacyRule,
    kind: ImportDiagnosticKind,
    reason: String,
) -> AxiomImportResult {
    AxiomImportResult::Skipped(ImportDiagnostic {
        rule_id: rule.rule_id.clone(),
        kind,
        reason,
    })
}

/// Translates normalized legacy rules into `DetectorIr` (never into authority).
#[derive(Debug, Clone, Copy, Default)]
pub struct AxiomDetectorTranslator;

impl AxiomDetectorTranslator {
    /// Translate one rule.
    pub fn translate(rule: &NormalizedLegacyRule) -> AxiomImportResult {
        let Some(id) = detector_id_for(rule) else {
            return skip(
                rule,
                ImportDiagnosticKind::Unmappable,
                "rule id/system cannot form a namespaced detector id".to_string(),
            );
        };
        let Some(kind) = kind_for(rule) else {
            return skip(
                rule,
                ImportDiagnosticKind::Unmappable,
                "category/rule id cannot form a namespaced finding kind".to_string(),
            );
        };

        let (requires, steps, capability) = match &rule.detection {
            LegacyDetection::None => {
                return skip(
                    rule,
                    ImportDiagnosticKind::MetadataOnly,
                    "the legacy rule carries no executable semantics (catalogue metadata only)"
                        .to_string(),
                );
            }
            LegacyDetection::Unsupported { reason } => {
                return skip(
                    rule,
                    ImportDiagnosticKind::UnsupportedAnalysis,
                    reason.clone(),
                );
            }
            LegacyDetection::AstPattern { subject } => {
                let Ok(subject) = SubjectPattern::new(subject.clone()) else {
                    return skip(
                        rule,
                        ImportDiagnosticKind::Unmappable,
                        format!("`{subject}` is not a namespaced subject (`ns.name`)"),
                    );
                };
                (
                    [AnalysisCapability::AstPattern],
                    vec![
                        DetectorStep::Match {
                            subject: subject.clone(),
                        },
                        DetectorStep::Produce { kind },
                    ],
                    "ast",
                )
            }
            LegacyDetection::Reachability { source, sink } => {
                let (Ok(source), Ok(sink)) = (
                    SubjectPattern::new(source.clone()),
                    SubjectPattern::new(sink.clone()),
                ) else {
                    return skip(
                        rule,
                        ImportDiagnosticKind::Unmappable,
                        "source/sink are not namespaced subjects (`ns.name`)".to_string(),
                    );
                };
                (
                    [AnalysisCapability::GraphQuery],
                    vec![
                        DetectorStep::Match {
                            subject: source.clone(),
                        },
                        DetectorStep::Flow {
                            source,
                            sink,
                            max_hops: None,
                        },
                        DetectorStep::Produce { kind },
                    ],
                    "graph",
                )
            }
            LegacyDetection::TaintFlow {
                source,
                sink,
                sanitizers,
            } => {
                let (Ok(source), Ok(sink)) = (
                    SubjectPattern::new(source.clone()),
                    SubjectPattern::new(sink.clone()),
                ) else {
                    return skip(
                        rule,
                        ImportDiagnosticKind::Unmappable,
                        "source/sink are not namespaced subjects (`ns.name`)".to_string(),
                    );
                };
                // Canonical IR order: MATCH, FLOW, EXCLUDE…, PRODUCE.
                let mut steps = vec![
                    DetectorStep::Match {
                        subject: source.clone(),
                    },
                    DetectorStep::Flow {
                        source,
                        sink,
                        max_hops: None,
                    },
                ];
                for sanitizer in sanitizers {
                    let Ok(path_contains) = SubjectPattern::new(sanitizer.clone()) else {
                        return skip(
                            rule,
                            ImportDiagnosticKind::Unmappable,
                            format!("`{sanitizer}` is not a namespaced sanitizer subject"),
                        );
                    };
                    steps.push(DetectorStep::Exclude { path_contains });
                }
                steps.push(DetectorStep::Produce { kind });
                ([AnalysisCapability::Dataflow], steps, "dataflow")
            }
        };

        let ir = DetectorIr {
            id,
            name: rule.name.clone(),
            policy: policy_for(rule.severity),
            // Capability is derived from the legacy semantics — never declared
            // just to satisfy the planner.
            requires: requires.into_iter().collect(),
            // The import claims nothing: admission normalises to Candidate.
            authority: DetectorAuthority::Candidate,
            steps,
        };

        if let Err(err) = ir.validate() {
            return skip(
                rule,
                ImportDiagnosticKind::Unmappable,
                format!("translated definition for class `{capability}` is invalid: {err}"),
            );
        }

        AxiomImportResult::Translated(ImportedDetectorDefinition {
            ir,
            legacy: LegacyRuleProvenance {
                system: rule.system.clone(),
                legacy_rule_id: rule.rule_id.clone(),
                legacy_language: rule.language.clone(),
                legacy_category: rule.category.clone(),
                legacy_severity: rule.severity.as_str().to_string(),
                source_revision: rule.source_revision.clone(),
            },
        })
    }
}

// ============================================================================
// Reader + batch importer
// ============================================================================

/// Reads legacy rules into normalized form.
///
/// The archived Axiom corpus plugs in here; the importer never touches the
/// legacy crate's architecture or authority model.
pub trait AxiomRuleReader {
    /// Read one rule by id.
    fn read(&self, rule_id: &str) -> Result<NormalizedLegacyRule, String>;
}

/// Result of importing a batch of legacy rules.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportReport {
    /// Successfully translated definitions (not admitted).
    pub translated: Vec<ImportedDetectorDefinition>,
    /// Skipped rules with stable diagnostics.
    pub skipped: Vec<ImportDiagnostic>,
}

/// Batch importer over a reader. **Creates no permits.**
pub struct AxiomImporter<R> {
    reader: R,
}

impl<R: AxiomRuleReader> AxiomImporter<R> {
    /// Construct an importer over a reader.
    pub fn new(reader: R) -> Self {
        Self { reader }
    }

    /// Import the given rule ids.
    pub fn import(&self, rule_ids: &[&str]) -> ImportReport {
        let mut report = ImportReport::default();
        for rule_id in rule_ids {
            match self.reader.read(rule_id) {
                Ok(rule) => match AxiomDetectorTranslator::translate(&rule) {
                    AxiomImportResult::Translated(def) => report.translated.push(def),
                    AxiomImportResult::Skipped(diag) => report.skipped.push(diag),
                },
                Err(reason) => report.skipped.push(ImportDiagnostic {
                    rule_id: (*rule_id).to_string(),
                    kind: ImportDiagnosticKind::Unmappable,
                    reason,
                }),
            }
        }
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(detection: LegacyDetection, severity: LegacySeverity) -> NormalizedLegacyRule {
        NormalizedLegacyRule {
            system: "axiom".to_string(),
            rule_id: "S1234".to_string(),
            name: "weak hash usage".to_string(),
            language: "rust".to_string(),
            category: "security".to_string(),
            severity,
            source_revision: "axiom-1.0".to_string(),
            detection,
        }
    }

    fn translated(rule: &NormalizedLegacyRule) -> ImportedDetectorDefinition {
        match AxiomDetectorTranslator::translate(rule) {
            AxiomImportResult::Translated(def) => def,
            AxiomImportResult::Skipped(d) => panic!("expected translation, got {d:?}"),
        }
    }

    fn diagnostic(rule: &NormalizedLegacyRule) -> ImportDiagnostic {
        match AxiomDetectorTranslator::translate(rule) {
            AxiomImportResult::Skipped(d) => d,
            AxiomImportResult::Translated(def) => panic!("expected a skip, got {def:?}"),
        }
    }

    // U-A1 — a supported AST rule becomes a valid DetectorIr.
    #[test]
    fn u_a1_ast_rule_translates_to_a_valid_detector_ir() {
        let def = translated(&rule(
            LegacyDetection::AstPattern {
                subject: "security.md5_usage".to_string(),
            },
            LegacySeverity::Major,
        ));
        assert_eq!(def.ir.id.as_str(), "axiom.s1234");
        assert_eq!(def.ir.name, "weak hash usage");
        assert_eq!(def.ir.policy.default_severity, FindingSeverity::Warning);
        assert_eq!(def.ir.policy.default_risk, RiskLevel::Medium);
        def.ir.validate().expect("translated IR must validate");
        assert_eq!(def.legacy.system, "axiom");
        assert_eq!(def.legacy.legacy_rule_id, "S1234");
        assert_eq!(def.legacy.legacy_severity, "MAJOR");
        assert_eq!(def.legacy.source_revision, "axiom-1.0");
        // Provenance lives beside the IR, not inside it.
        assert!(def.ir.steps.iter().any(|s| matches!(
            s,
            DetectorStep::Produce { kind } if kind.as_str() == "security.s1234"
        )));
    }

    // U-A4 — the capability is derived from the legacy semantics.
    #[test]
    fn u_a4_capability_is_derived_from_semantics() {
        let ast = translated(&rule(
            LegacyDetection::AstPattern {
                subject: "security.md5_usage".to_string(),
            },
            LegacySeverity::Minor,
        ));
        assert_eq!(
            ast.ir.requires,
            [AnalysisCapability::AstPattern].into_iter().collect()
        );

        let graph = translated(&rule(
            LegacyDetection::Reachability {
                source: "endpoint.http".to_string(),
                sink: "persistence.write".to_string(),
            },
            LegacySeverity::Major,
        ));
        assert_eq!(
            graph.ir.requires,
            [AnalysisCapability::GraphQuery].into_iter().collect()
        );

        let dataflow = translated(&rule(
            LegacyDetection::TaintFlow {
                source: "security.user_input".to_string(),
                sink: "security.sql_execution".to_string(),
                sanitizers: vec!["security.sanitizer".to_string()],
            },
            LegacySeverity::Blocker,
        ));
        assert_eq!(
            dataflow.ir.requires,
            [AnalysisCapability::Dataflow].into_iter().collect()
        );
        // The translated IR is valid (V9/V10/V11 all satisfied).
        dataflow.ir.validate().expect("translated IR must validate");
    }

    // U-A2 — legacy BLOCKER never yields authority; it only maps to policy.
    #[test]
    fn u_a2_legacy_blocker_maps_to_policy_not_authority() {
        let def = translated(&rule(
            LegacyDetection::AstPattern {
                subject: "security.md5_usage".to_string(),
            },
            LegacySeverity::Blocker,
        ));
        assert_eq!(
            def.ir.authority,
            DetectorAuthority::Candidate,
            "the import claims no authority"
        );
        assert_eq!(def.ir.policy.default_severity, FindingSeverity::Critical);
        assert_eq!(def.ir.policy.default_risk, RiskLevel::Critical);
        assert_eq!(def.legacy.legacy_severity, "BLOCKER");

        // And admission keeps it Candidate.
        let permit = crate::domain::findings::DetectorAdmission::admit(
            def.ir,
            "1.0.0",
            crate::domain::findings::AdmissionSource::Imported,
        )
        .unwrap();
        assert_eq!(
            permit.authority(),
            DetectorAuthority::Candidate,
            "a BLOCKER legacy rule must never arrive Gated"
        );
        assert!(!permit.can_block());
    }

    // U-A3 — metadata-only rules produce no invented detector.
    #[test]
    fn u_a3_metadata_only_rule_is_skipped() {
        let diag = diagnostic(&rule(LegacyDetection::None, LegacySeverity::Major));
        assert_eq!(diag.kind, ImportDiagnosticKind::MetadataOnly);
        assert_eq!(diag.rule_id, "S1234");
        assert!(!diag.reason.is_empty());
    }

    // U-A5 — unsupported analysis fails loud with a stable diagnostic.
    #[test]
    fn u_a5_unsupported_analysis_is_skipped_loudly() {
        let diag = diagnostic(&rule(
            LegacyDetection::Unsupported {
                reason: "requires interprocedural symbol resolution".to_string(),
            },
            LegacySeverity::Critical,
        ));
        assert_eq!(diag.kind, ImportDiagnosticKind::UnsupportedAnalysis);
        assert!(diag.reason.contains("interprocedural"));

        // A non-namespaced subject is unmappable rather than guessed.
        let diag = diagnostic(&rule(
            LegacyDetection::AstPattern {
                subject: "MethodParametersReassigned".to_string(),
            },
            LegacySeverity::Major,
        ));
        assert_eq!(diag.kind, ImportDiagnosticKind::Unmappable);
    }

    // U-A6 — the same rule/revision yields the same semantic digest.
    #[test]
    fn u_a6_stable_translation() {
        let detection = LegacyDetection::TaintFlow {
            source: "security.user_input".to_string(),
            sink: "security.sql_execution".to_string(),
            sanitizers: vec!["security.sanitizer".to_string()],
        };
        let a = translated(&rule(detection.clone(), LegacySeverity::Major));
        let b = translated(&rule(detection, LegacySeverity::Major));
        assert_eq!(a.ir.semantic_digest(), b.ir.semantic_digest());
        assert_eq!(a, b);
    }

    // U-A7 — severity changes policy + semantic digest, not logic digest.
    #[test]
    fn u_a7_severity_change_moves_policy_not_logic() {
        let detection = LegacyDetection::AstPattern {
            subject: "security.md5_usage".to_string(),
        };
        let minor = translated(&rule(detection.clone(), LegacySeverity::Minor));
        let major = translated(&rule(detection, LegacySeverity::Major));

        assert_eq!(
            minor.ir.logic_digest(),
            major.ir.logic_digest(),
            "the detection logic is unchanged"
        );
        assert_ne!(minor.ir.policy_digest(), major.ir.policy_digest());
        assert_ne!(minor.ir.semantic_digest(), major.ir.semantic_digest());
    }

    #[test]
    fn batch_import_reports_translated_and_skipped() {
        struct Corpus;
        impl AxiomRuleReader for Corpus {
            fn read(&self, rule_id: &str) -> Result<NormalizedLegacyRule, String> {
                match rule_id {
                    "A" => {
                        let mut r = rule(
                            LegacyDetection::AstPattern {
                                subject: "security.md5_usage".to_string(),
                            },
                            LegacySeverity::Major,
                        );
                        r.rule_id = "A".to_string();
                        Ok(r)
                    }
                    "E" => {
                        let mut r = rule(LegacyDetection::None, LegacySeverity::Info);
                        r.rule_id = "E".to_string();
                        Ok(r)
                    }
                    "D" => {
                        let mut r = rule(
                            LegacyDetection::Unsupported {
                                reason: "global aggregate".to_string(),
                            },
                            LegacySeverity::Critical,
                        );
                        r.rule_id = "D".to_string();
                        Ok(r)
                    }
                    other => Err(format!("unknown rule `{other}`")),
                }
            }
        }

        let report = AxiomImporter::new(Corpus).import(&["A", "E", "D", "missing"]);
        assert_eq!(report.translated.len(), 1);
        assert_eq!(report.translated[0].ir.id.as_str(), "axiom.a");
        assert_eq!(report.skipped.len(), 3);
        assert_eq!(report.skipped[0].kind, ImportDiagnosticKind::MetadataOnly);
        assert_eq!(
            report.skipped[1].kind,
            ImportDiagnosticKind::UnsupportedAnalysis
        );
        assert_eq!(report.skipped[2].kind, ImportDiagnosticKind::Unmappable);
    }
}
