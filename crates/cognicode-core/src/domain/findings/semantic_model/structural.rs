//! Structural security families: facts that need no source-to-sink flow.
//!
//! Hardcoded secrets and `unsafe` code are properties of a construct, not of a
//! flow, so they go through the AST backend and carry class C evidence at
//! best. They are modeled separately from the taint [`Weakness`](super::Weakness)
//! catalog because there is no source, sink or sanitizer to model.
//!
//! `unsafe` Rust is a **risk marker**: it says where the compiler's memory
//! guarantees are suspended, not that a vulnerability exists. The detector's
//! policy keeps it low risk and informational, and the pack never lets it
//! block.
//!
//! Pure domain: no I/O.

use serde::{Deserialize, Serialize};

use super::catalog::SourceLanguage;
use crate::domain::findings::{
    AnalysisCapability, AstConstruct, AstInput, AstUnit, DetectorAuthority, DetectorFindingPolicy,
    DetectorId, DetectorIr, DetectorStep, FindingKind, FindingSeverity, GroundingRef, RiskLevel,
    SubjectPattern,
};

/// Subject carried by a hardcoded credential literal.
pub const HARDCODED_SECRET_SUBJECT: &str = "security.hardcoded_secret";
/// Subject carried by an `unsafe` construct.
pub const UNSAFE_CODE_SUBJECT: &str = "security.unsafe_code";
/// The finding kind of the hardcoded-secret detector.
pub const HARDCODED_SECRET_KIND: &str = "security.hardcoded_secret";
/// The finding kind of the unsafe-code detector: a risk, not a vulnerability.
pub const UNSAFE_CODE_KIND: &str = "security.unsafe_code_risk";
/// Subject carried by a verifiable public-exposure misconfiguration.
pub const IAC_PUBLIC_EXPOSURE_SUBJECT: &str = "security.iac.public_exposure";
/// Subject carried by a verifiable missing-encryption misconfiguration.
pub const IAC_ENCRYPTION_SUBJECT: &str = "security.iac.encryption";
/// Subject carried by a verifiable excessive-permission misconfiguration.
pub const IAC_PERMISSIONS_SUBJECT: &str = "security.iac.permissions";
/// The finding kind of the public-exposure detector.
pub const IAC_PUBLIC_EXPOSURE_KIND: &str = "security.iac_public_exposure";
/// The finding kind of the missing-encryption detector.
pub const IAC_ENCRYPTION_KIND: &str = "security.iac_missing_encryption";
/// The finding kind of the excessive-permission detector.
pub const IAC_PERMISSIONS_KIND: &str = "security.iac_excessive_permissions";

/// A structural family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StructuralFamily {
    /// A credential literal bound to a secret-like name (CWE-798).
    HardcodedSecret,
    /// An `unsafe` construct (Rust).
    UnsafeCode,
    /// An infrastructure resource reachable from outside its boundary
    /// (CWE-284), established from known configuration values.
    InfraPublicExposure,
    /// Infrastructure data left unencrypted (CWE-311).
    InfraEncryption,
    /// Infrastructure access granted beyond need (CWE-732).
    InfraPermissions,
}

impl StructuralFamily {
    /// Every structural family, in a stable order.
    pub const ALL: [Self; 5] = [
        Self::HardcodedSecret,
        Self::UnsafeCode,
        Self::InfraPublicExposure,
        Self::InfraEncryption,
        Self::InfraPermissions,
    ];

    /// Whether the family reads infrastructure configuration rather than
    /// program text.
    pub fn is_infrastructure(self) -> bool {
        matches!(
            self,
            Self::InfraPublicExposure | Self::InfraEncryption | Self::InfraPermissions
        )
    }

    /// The CWE the family maps to, when it maps to one. `unsafe` maps to none:
    /// it is not a weakness by itself.
    pub fn cwe(self) -> Option<&'static str> {
        match self {
            Self::HardcodedSecret => Some("CWE-798"),
            Self::UnsafeCode => None,
            Self::InfraPublicExposure => Some("CWE-284"),
            Self::InfraEncryption => Some("CWE-311"),
            Self::InfraPermissions => Some("CWE-732"),
        }
    }

    /// The AST subject the family matches.
    pub fn subject(self) -> &'static str {
        match self {
            Self::HardcodedSecret => HARDCODED_SECRET_SUBJECT,
            Self::UnsafeCode => UNSAFE_CODE_SUBJECT,
            Self::InfraPublicExposure => IAC_PUBLIC_EXPOSURE_SUBJECT,
            Self::InfraEncryption => IAC_ENCRYPTION_SUBJECT,
            Self::InfraPermissions => IAC_PERMISSIONS_SUBJECT,
        }
    }

    /// The finding kind the family's detector produces.
    pub fn finding_kind(self) -> &'static str {
        match self {
            Self::HardcodedSecret => HARDCODED_SECRET_KIND,
            Self::UnsafeCode => UNSAFE_CODE_KIND,
            Self::InfraPublicExposure => IAC_PUBLIC_EXPOSURE_KIND,
            Self::InfraEncryption => IAC_ENCRYPTION_KIND,
            Self::InfraPermissions => IAC_PERMISSIONS_KIND,
        }
    }

    /// Severity and risk of the family's findings.
    ///
    /// `unsafe` is informational and low risk: it flags where to look, it does
    /// not assert a defect.
    pub fn policy(self) -> DetectorFindingPolicy {
        match self {
            Self::HardcodedSecret => {
                DetectorFindingPolicy::new(FindingSeverity::Critical, RiskLevel::High)
            }
            Self::UnsafeCode => DetectorFindingPolicy::new(FindingSeverity::Info, RiskLevel::Low),
            Self::InfraPublicExposure | Self::InfraPermissions => {
                DetectorFindingPolicy::new(FindingSeverity::Critical, RiskLevel::High)
            }
            Self::InfraEncryption => {
                DetectorFindingPolicy::new(FindingSeverity::Warning, RiskLevel::High)
            }
        }
    }

    /// Whether the family has a model for `language`. Coverage is stated per
    /// language, never assumed from another one.
    pub fn supports(self, language: SourceLanguage) -> bool {
        match self {
            Self::HardcodedSecret => matches!(
                language,
                SourceLanguage::Rust
                    | SourceLanguage::Java
                    | SourceLanguage::Kotlin
                    | SourceLanguage::Go
                    | SourceLanguage::Terraform
            ),
            Self::UnsafeCode => language == SourceLanguage::Rust,
            Self::InfraPublicExposure | Self::InfraEncryption | Self::InfraPermissions => {
                language == SourceLanguage::Terraform
            }
        }
    }

    /// The detector of the family: one `MATCH` over its subject.
    pub fn detector(self) -> DetectorIr {
        let kind = self.finding_kind();
        DetectorIr {
            id: DetectorId::new(kind).expect("constant detector id is valid"),
            name: format!("{kind} structural detector"),
            policy: self.policy(),
            requires: [AnalysisCapability::AstPattern].into_iter().collect(),
            authority: DetectorAuthority::Candidate,
            steps: vec![
                DetectorStep::Match {
                    subject: SubjectPattern::new(self.subject())
                        .expect("constant subject is valid"),
                },
                DetectorStep::Produce {
                    kind: FindingKind::new(kind).expect("constant kind is valid"),
                },
            ],
        }
    }
}

/// The syntactic form of an `unsafe` construct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnsafeConstruct {
    /// `unsafe { .. }`.
    Block,
    /// `unsafe fn`.
    Function,
    /// `unsafe impl`.
    Impl,
    /// `unsafe trait`.
    Trait,
}

impl UnsafeConstruct {
    fn name(self) -> &'static str {
        match self {
            Self::Block => "unsafe block",
            Self::Function => "unsafe fn",
            Self::Impl => "unsafe impl",
            Self::Trait => "unsafe trait",
        }
    }
}

/// A structural fact observed in a unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StructuralFact {
    /// A string literal bound to a name (`let password = "..."`).
    StringBinding {
        /// The bound name.
        name: String,
        /// The literal's text. It is inspected, never copied into a finding.
        value: String,
    },
    /// An `unsafe` construct.
    Unsafe {
        /// Its syntactic form.
        construct: UnsafeConstruct,
        /// Whether a `SAFETY:` justification precedes it.
        safety_comment: bool,
    },
    /// A risky configuration established from known values only: every value
    /// the check needed was known when it concluded.
    Misconfiguration {
        /// The infrastructure family it belongs to.
        family: StructuralFamily,
        /// The stable check id.
        check: String,
        /// The resource instance address.
        resource: String,
        /// The attribute path the check looked at.
        attribute: String,
        /// What is wrong. Never carries a secret value.
        detail: String,
    },
    /// A check that could not decide because a value it needs is unknown. It
    /// never becomes a finding; it keeps the verdict from reading as clean.
    UnknownValue {
        /// The infrastructure family of the check.
        family: StructuralFamily,
        /// The resource instance address.
        resource: String,
        /// The attribute path.
        attribute: String,
        /// Why the value is unknown.
        causes: Vec<String>,
    },
}

/// One structural fact with its position and provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuralObservation {
    /// 1-based source line.
    pub line: u32,
    /// The canonical fact the observation was projected from, if any. Without
    /// it the finding is explainable but cannot open a gate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grounding: Option<GroundingRef>,
    /// The fact.
    pub fact: StructuralFact,
}

/// One source file as observed for structural families.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedUnit {
    /// Source path.
    pub path: String,
    /// The language of the file.
    pub language: SourceLanguage,
    /// Facts in source order.
    pub facts: Vec<StructuralObservation>,
}

/// The outcome of classifying a unit for one structural family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuralClassification {
    /// The family is modeled for the language; the view feeds the AST backend.
    /// An empty view means nothing matched, which is a scoped negative.
    Modeled(AstInput),
    /// The family has no model for the language: nothing was analysed.
    Unsupported {
        /// The language.
        language: SourceLanguage,
        /// The family.
        family: StructuralFamily,
    },
}

/// Classify `unit` for `family`.
pub fn classify_structural(
    family: StructuralFamily,
    unit: &ObservedUnit,
) -> StructuralClassification {
    if !family.supports(unit.language) {
        return StructuralClassification::Unsupported {
            language: unit.language,
            family,
        };
    }
    let subject = SubjectPattern::new(family.subject()).expect("constant subject is valid");
    let constructs = unit
        .facts
        .iter()
        .filter_map(|observation| {
            let detail = match (family, &observation.fact) {
                (
                    StructuralFamily::HardcodedSecret,
                    StructuralFact::StringBinding { name, value },
                ) if is_hardcoded_secret(name, value) => format!(
                    "credential-like literal bound to `{name}` ({} characters)",
                    value.chars().count()
                ),
                (
                    StructuralFamily::UnsafeCode,
                    StructuralFact::Unsafe {
                        construct,
                        safety_comment,
                    },
                ) => format!(
                    "{}, {}",
                    construct.name(),
                    if *safety_comment {
                        "SAFETY justification present"
                    } else {
                        "no SAFETY justification"
                    }
                ),
                (
                    family,
                    StructuralFact::Misconfiguration {
                        family: fact_family,
                        check,
                        resource,
                        attribute,
                        detail,
                    },
                ) if family == *fact_family => {
                    format!("{check}: {detail} ({resource}, `{attribute}`)")
                }
                _ => return None,
            };
            Some(AstConstruct {
                subject: subject.clone(),
                line: observation.line,
                detail,
                grounding: observation.grounding,
            })
        })
        .collect();
    StructuralClassification::Modeled(AstInput {
        units: vec![AstUnit {
            path: unit.path.clone(),
            constructs,
        }],
    })
}

/// The checks of `family` that could not decide in `unit` because a value they
/// need is unknown, as stable sorted descriptions. A run that finds nothing
/// while this is non-empty has not shown the unit clean.
pub fn unresolved_checks(family: StructuralFamily, unit: &ObservedUnit) -> Vec<String> {
    let mut unresolved: Vec<String> = unit
        .facts
        .iter()
        .filter_map(|observation| match &observation.fact {
            StructuralFact::UnknownValue {
                family: fact_family,
                resource,
                attribute,
                causes,
            } if *fact_family == family => {
                Some(format!("{resource} `{attribute}`: {}", causes.join("; ")))
            }
            _ => None,
        })
        .collect();
    unresolved.sort();
    unresolved.dedup();
    unresolved
}

const SECRET_MARKERS: [&str; 7] = [
    "password",
    "passwd",
    "secret",
    "token",
    "api_key",
    "apikey",
    "private_key",
];

/// Name parts that say the value *locates* a secret instead of being one.
const LOCATOR_MARKERS: [&str; 7] = ["env", "path", "file", "header", "prompt", "label", "url"];

const PLACEHOLDERS: [&str; 6] = [
    "changeme",
    "password",
    "example",
    "placeholder",
    "dummy",
    "test",
];

const MIN_SECRET_LEN: usize = 8;

/// Whether `value`, bound to `name`, looks like a credential committed to the
/// source: a secret-like name with an opaque literal that is not a placeholder.
pub fn is_hardcoded_secret(name: &str, value: &str) -> bool {
    let name = name.to_ascii_lowercase();
    if !SECRET_MARKERS.iter().any(|m| name.contains(m))
        || LOCATOR_MARKERS.iter().any(|m| name.contains(m))
    {
        return false;
    }
    !is_placeholder(value)
}

fn is_placeholder(value: &str) -> bool {
    let value = value.trim();
    let lower = value.to_ascii_lowercase();
    value.chars().count() < MIN_SECRET_LEN
        || value.starts_with("${")
        || value.starts_with("{{")
        || value.starts_with('<')
        || value.starts_with('%')
        || PLACEHOLDERS.contains(&lower.as_str())
        || value.chars().all(|c| value.starts_with(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(name: &str, value: &str) -> StructuralObservation {
        StructuralObservation {
            line: 3,
            grounding: None,
            fact: StructuralFact::StringBinding {
                name: name.to_string(),
                value: value.to_string(),
            },
        }
    }

    fn unit(language: SourceLanguage, facts: Vec<StructuralObservation>) -> ObservedUnit {
        ObservedUnit {
            path: "src/x".to_string(),
            language,
            facts,
        }
    }

    fn constructs(classification: StructuralClassification) -> Vec<AstConstruct> {
        match classification {
            StructuralClassification::Modeled(ast) => {
                ast.units.into_iter().flat_map(|u| u.constructs).collect()
            }
            other => panic!("expected Modeled, got {other:?}"),
        }
    }

    #[test]
    fn a_secret_like_name_with_an_opaque_literal_is_flagged_without_echoing_it() {
        let found = constructs(classify_structural(
            StructuralFamily::HardcodedSecret,
            &unit(
                SourceLanguage::Go,
                vec![binding("dbPassword", "s3cr3t-Value-42")],
            ),
        ));
        assert_eq!(found.len(), 1);
        assert!(found[0].detail.contains("dbPassword"));
        assert!(!found[0].detail.contains("s3cr3t-Value-42"));
    }

    #[test]
    fn placeholders_short_values_and_locators_are_not_flagged() {
        for (name, value) in [
            ("password", "changeme"),
            ("api_key", "${API_KEY}"),
            ("token", "short"),
            ("secret", "xxxxxxxxxx"),
            ("password_file", "/run/secrets/db-password"),
            ("greeting", "hello-world-123"),
        ] {
            let found = constructs(classify_structural(
                StructuralFamily::HardcodedSecret,
                &unit(SourceLanguage::Rust, vec![binding(name, value)]),
            ));
            assert!(found.is_empty(), "{name}={value}");
        }
    }

    #[test]
    fn unsafe_is_reported_with_its_justification_state() {
        let facts = [true, false]
            .into_iter()
            .map(|safety_comment| StructuralObservation {
                line: 1,
                grounding: None,
                fact: StructuralFact::Unsafe {
                    construct: UnsafeConstruct::Block,
                    safety_comment,
                },
            })
            .collect();
        let found = constructs(classify_structural(
            StructuralFamily::UnsafeCode,
            &unit(SourceLanguage::Rust, facts),
        ));
        assert!(found[0].detail.contains("SAFETY justification present"));
        assert!(found[1].detail.contains("no SAFETY justification"));
    }

    #[test]
    fn a_family_without_a_model_for_the_language_is_unsupported_not_empty() {
        for language in [
            SourceLanguage::Java,
            SourceLanguage::Kotlin,
            SourceLanguage::Go,
        ] {
            assert_eq!(
                classify_structural(StructuralFamily::UnsafeCode, &unit(language, vec![])),
                StructuralClassification::Unsupported {
                    language,
                    family: StructuralFamily::UnsafeCode
                }
            );
        }
        assert!(matches!(
            classify_structural(
                StructuralFamily::HardcodedSecret,
                &unit(SourceLanguage::Python, vec![])
            ),
            StructuralClassification::Unsupported { .. }
        ));
    }

    #[test]
    fn a_fact_of_another_family_is_ignored() {
        let found = constructs(classify_structural(
            StructuralFamily::UnsafeCode,
            &unit(
                SourceLanguage::Rust,
                vec![binding("password", "hunter2hunter2")],
            ),
        ));
        assert!(found.is_empty());
    }

    #[test]
    fn detectors_validate_and_unsafe_is_a_low_risk_marker() {
        for family in StructuralFamily::ALL {
            assert_eq!(family.detector().validate(), Ok(()));
        }
        let policy = StructuralFamily::UnsafeCode.policy();
        assert_eq!(policy.default_risk, RiskLevel::Low);
        assert_eq!(policy.default_severity, FindingSeverity::Info);
        assert_eq!(StructuralFamily::UnsafeCode.cwe(), None);
    }

    fn misconfiguration(family: StructuralFamily) -> StructuralObservation {
        StructuralObservation {
            line: 9,
            grounding: None,
            fact: StructuralFact::Misconfiguration {
                family,
                check: "open_ingress".to_string(),
                resource: "aws_security_group.web".to_string(),
                attribute: "ingress[0].cidr_blocks".to_string(),
                detail: "ingress from anywhere".to_string(),
            },
        }
    }

    fn unknown_value(family: StructuralFamily, resource: &str) -> StructuralObservation {
        StructuralObservation {
            line: 4,
            grounding: None,
            fact: StructuralFact::UnknownValue {
                family,
                resource: resource.to_string(),
                attribute: "acl".to_string(),
                causes: vec!["variable `var.acl` is not bound".to_string()],
            },
        }
    }

    #[test]
    fn a_misconfiguration_becomes_a_construct_only_for_its_own_family() {
        let facts = vec![misconfiguration(StructuralFamily::InfraPublicExposure)];
        let own = constructs(classify_structural(
            StructuralFamily::InfraPublicExposure,
            &unit(SourceLanguage::Terraform, facts.clone()),
        ));
        assert_eq!(own.len(), 1);
        assert_eq!(own[0].line, 9);
        assert!(own[0].detail.contains("aws_security_group.web"));
        let other = constructs(classify_structural(
            StructuralFamily::InfraEncryption,
            &unit(SourceLanguage::Terraform, facts),
        ));
        assert!(other.is_empty());
    }

    #[test]
    fn an_unknown_value_is_never_a_construct_but_is_reported_as_unresolved() {
        let terraform = unit(
            SourceLanguage::Terraform,
            vec![
                unknown_value(StructuralFamily::InfraPublicExposure, "aws_s3_bucket.b"),
                unknown_value(StructuralFamily::InfraPublicExposure, "aws_s3_bucket.b"),
                unknown_value(StructuralFamily::InfraEncryption, "aws_ebs_volume.d"),
            ],
        );
        assert!(
            constructs(classify_structural(
                StructuralFamily::InfraPublicExposure,
                &terraform
            ))
            .is_empty()
        );
        let unresolved = unresolved_checks(StructuralFamily::InfraPublicExposure, &terraform);
        assert_eq!(unresolved.len(), 1, "duplicates collapse");
        assert!(unresolved[0].contains("aws_s3_bucket.b"));
        assert!(unresolved_checks(StructuralFamily::HardcodedSecret, &terraform).is_empty());
    }

    #[test]
    fn infrastructure_families_are_modeled_for_terraform_only_and_secrets_join_them() {
        for family in StructuralFamily::ALL
            .into_iter()
            .filter(|f| f.is_infrastructure())
        {
            for language in SourceLanguage::ALL {
                assert_eq!(
                    family.supports(language),
                    language == SourceLanguage::Terraform,
                    "{family:?} {language:?}"
                );
            }
            assert!(family.cwe().is_some());
        }
        assert!(StructuralFamily::HardcodedSecret.supports(SourceLanguage::Terraform));
        assert!(!StructuralFamily::UnsafeCode.supports(SourceLanguage::Terraform));
    }
}
