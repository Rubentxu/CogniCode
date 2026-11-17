//! The security pack: eight families, one capability matrix.
//!
//! The pack names the families it covers, what kind of claim each makes
//! ([`FindingNature`]), the evidence a finding needs before it may block
//! ([`BlockingLevel`]), and publishes a [`CapabilityMatrix`] that states, per
//! language and family, what is analysed, with which algorithm, at which
//! version, up to which evidence class, and where it stops.
//!
//! The matrix is **derived**, not written by hand: support comes from the
//! catalog's adapters and from what the registered backends really offer, so a
//! language without an adapter or a registry without a dataflow backend shows
//! up as unsupported instead of as a stale promise.
//!
//! Pure domain: no I/O.

use serde::{Deserialize, Serialize};

use super::catalog::{SemanticModelCatalog, SourceLanguage, Weakness};
use super::rule::{AnalysisReach, taint_detector};
use super::structural::StructuralFamily;
use crate::domain::findings::{BackendRegistry, DetectorIr, EvidenceClass, FindingGate, RiskLevel};

/// Version of the pack: its detectors, catalog tables and limits. Bump it when
/// any of them changes what a result means.
pub const PACK_VERSION: &str = "1.1.0";

/// What a finding of a family claims.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingNature {
    /// A weakness an attacker can exploit.
    Vulnerability,
    /// A place that deserves review; not a defect by itself.
    Risk,
}

/// When a finding of a family may block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockingLevel {
    /// Blocks only with a gated detector, grounded evidence of at least
    /// `min_evidence_class` and a finding risk of at least `min_risk`.
    Gated {
        /// Weakest evidence class accepted.
        min_evidence_class: EvidenceClass,
        /// Lowest risk accepted.
        min_risk: RiskLevel,
    },
    /// Never blocks, whatever the evidence: the family reports risk only.
    AdvisoryOnly,
}

impl BlockingLevel {
    /// The gate a finding must clear, or `None` when it can never block.
    pub fn gate(self) -> Option<FindingGate> {
        match self {
            Self::Gated {
                min_evidence_class,
                min_risk,
            } => Some(FindingGate::new(min_evidence_class, min_risk)),
            Self::AdvisoryOnly => None,
        }
    }
}

/// A security family of the pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityFamily {
    /// CWE-89.
    SqlInjection,
    /// CWE-78.
    CommandInjection,
    /// CWE-22.
    PathTraversal,
    /// CWE-798.
    HardcodedSecret,
    /// `unsafe` Rust.
    UnsafeCode,
    /// CWE-284 in infrastructure configuration.
    InfraPublicExposure,
    /// CWE-311 in infrastructure configuration.
    InfraEncryption,
    /// CWE-732 in infrastructure configuration.
    InfraPermissions,
}

impl SecurityFamily {
    /// Every family, in a stable order.
    pub const ALL: [Self; 8] = [
        Self::SqlInjection,
        Self::CommandInjection,
        Self::PathTraversal,
        Self::HardcodedSecret,
        Self::UnsafeCode,
        Self::InfraPublicExposure,
        Self::InfraEncryption,
        Self::InfraPermissions,
    ];

    /// The family of a taint weakness.
    pub fn of_weakness(weakness: Weakness) -> Self {
        match weakness {
            Weakness::SqlInjection => Self::SqlInjection,
            Weakness::CommandInjection => Self::CommandInjection,
            Weakness::PathTraversal => Self::PathTraversal,
        }
    }

    /// The family of a structural family.
    pub fn of_structural(family: StructuralFamily) -> Self {
        match family {
            StructuralFamily::HardcodedSecret => Self::HardcodedSecret,
            StructuralFamily::UnsafeCode => Self::UnsafeCode,
            StructuralFamily::InfraPublicExposure => Self::InfraPublicExposure,
            StructuralFamily::InfraEncryption => Self::InfraEncryption,
            StructuralFamily::InfraPermissions => Self::InfraPermissions,
        }
    }

    /// The taint weakness, when the family is a source-to-sink flow.
    pub fn weakness(self) -> Option<Weakness> {
        match self {
            Self::SqlInjection => Some(Weakness::SqlInjection),
            Self::CommandInjection => Some(Weakness::CommandInjection),
            Self::PathTraversal => Some(Weakness::PathTraversal),
            Self::HardcodedSecret
            | Self::UnsafeCode
            | Self::InfraPublicExposure
            | Self::InfraEncryption
            | Self::InfraPermissions => None,
        }
    }

    /// The structural family, when the family is a construct property.
    pub fn structural(self) -> Option<StructuralFamily> {
        match self {
            Self::HardcodedSecret => Some(StructuralFamily::HardcodedSecret),
            Self::UnsafeCode => Some(StructuralFamily::UnsafeCode),
            Self::InfraPublicExposure => Some(StructuralFamily::InfraPublicExposure),
            Self::InfraEncryption => Some(StructuralFamily::InfraEncryption),
            Self::InfraPermissions => Some(StructuralFamily::InfraPermissions),
            Self::SqlInjection | Self::CommandInjection | Self::PathTraversal => None,
        }
    }

    /// The CWE, when the family is a weakness by itself.
    pub fn cwe(self) -> Option<&'static str> {
        match (self.weakness(), self.structural()) {
            (Some(w), _) => Some(w.cwe()),
            (None, Some(s)) => s.cwe(),
            (None, None) => None,
        }
    }

    /// The finding kind the family produces.
    pub fn finding_kind(self) -> &'static str {
        match (self.weakness(), self.structural()) {
            (Some(w), _) => w.finding_kind(),
            (None, Some(s)) => s.finding_kind(),
            (None, None) => unreachable!("every family is taint or structural"),
        }
    }

    /// Whether findings are vulnerabilities or risks.
    pub fn nature(self) -> FindingNature {
        match self {
            Self::UnsafeCode => FindingNature::Risk,
            _ => FindingNature::Vulnerability,
        }
    }

    /// The evidence level the family declares for blocking.
    ///
    /// Taint families need a dataflow path (class B); secrets and
    /// infrastructure misconfigurations are construct matches over known
    /// values (class C); `unsafe` never blocks.
    pub fn blocking(self) -> BlockingLevel {
        match self {
            Self::SqlInjection | Self::CommandInjection | Self::PathTraversal => {
                BlockingLevel::Gated {
                    min_evidence_class: EvidenceClass::B,
                    min_risk: RiskLevel::High,
                }
            }
            Self::HardcodedSecret
            | Self::InfraPublicExposure
            | Self::InfraEncryption
            | Self::InfraPermissions => BlockingLevel::Gated {
                min_evidence_class: EvidenceClass::C,
                min_risk: RiskLevel::High,
            },
            Self::UnsafeCode => BlockingLevel::AdvisoryOnly,
        }
    }

    /// The detector of the family. Only taint families depend on `reach`.
    pub fn detector(self, reach: AnalysisReach) -> DetectorIr {
        match (self.weakness(), self.structural()) {
            (Some(w), _) => taint_detector(w, reach),
            (None, Some(s)) => s.detector(),
            (None, None) => unreachable!("every family is taint or structural"),
        }
    }

    /// The algorithm that produces the family's findings.
    pub fn algorithm(self) -> &'static str {
        if self.weakness().is_some() {
            "forward-taint"
        } else {
            "ast-pattern"
        }
    }
}

/// What a pack run may claim. There is deliberately no "clean": a negative is
/// always scoped, and a case the pack cannot decide says so.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum PackVerdict {
    /// Findings were produced.
    Found {
        /// Whether they are vulnerabilities or risks.
        nature: FindingNature,
        /// How many.
        findings: usize,
    },
    /// Nothing was found, within the stated scope only.
    NotObserved {
        /// What was examined, e.g. the flow reach.
        within: String,
    },
    /// Nothing was found, but calls the model does not know remain: a flow may
    /// hide behind them.
    Unknown {
        /// The callees without a model, sorted.
        unresolved: Vec<String>,
    },
    /// The family has no model for the language: nothing was analysed.
    Unsupported {
        /// Why.
        reason: String,
    },
}

/// How many API models of each role a language has for a family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCounts {
    /// Sources of external data (shared by every taint family).
    pub sources: usize,
    /// Sinks of the family.
    pub sinks: usize,
    /// Sanitizers of the family.
    pub sanitizers: usize,
}

/// Whether a family is analysed for a language, and how.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Support {
    /// Analysed.
    Supported {
        /// The backend that runs the family.
        backend: String,
        /// The strongest evidence class that backend can produce.
        evidence_ceiling: EvidenceClass,
        /// Detector version.
        detector_version: String,
        /// Digest of the detector's logic, so a result names the exact rule.
        logic_digest: String,
        /// The flow reach of taint families; `None` for structural ones.
        reach: Option<AnalysisReach>,
        /// Whether the declared blocking level is reachable with that ceiling.
        can_block: bool,
        /// API model counts for taint families.
        models: Option<ModelCounts>,
    },
    /// Not analysed: a result for this pair must say so, never "clean".
    Unsupported {
        /// Why.
        reason: String,
    },
}

/// One cell of the matrix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityEntry {
    /// The language.
    pub language: SourceLanguage,
    /// The family.
    pub family: SecurityFamily,
    /// The algorithm, by name.
    pub algorithm: String,
    /// Support state.
    pub support: Support,
    /// What a finding of this family claims.
    pub nature: FindingNature,
    /// When a finding of this family may block.
    pub blocking: BlockingLevel,
    /// Where the analysis stops. A negative result never claims more.
    pub limits: Vec<String>,
}

/// The published capability matrix of the pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityMatrix {
    /// Pack version the matrix describes.
    pub pack_version: String,
    /// One entry per language and family, languages first.
    pub entries: Vec<CapabilityEntry>,
}

impl CapabilityMatrix {
    /// Derive the matrix from what is actually available: the catalog's
    /// adapters and the backends the registry can plan.
    pub fn derive(catalog: &SemanticModelCatalog, registry: &BackendRegistry) -> Self {
        let entries = SourceLanguage::ALL
            .into_iter()
            .flat_map(|language| {
                SecurityFamily::ALL
                    .into_iter()
                    .map(move |family| (language, family))
            })
            .map(|(language, family)| entry(catalog, registry, language, family))
            .collect();
        Self {
            pack_version: PACK_VERSION.to_string(),
            entries,
        }
    }

    /// The cell of `language` and `family`.
    pub fn get(
        &self,
        language: SourceLanguage,
        family: SecurityFamily,
    ) -> Option<&CapabilityEntry> {
        self.entries
            .iter()
            .find(|e| e.language == language && e.family == family)
    }
}

fn entry(
    catalog: &SemanticModelCatalog,
    registry: &BackendRegistry,
    language: SourceLanguage,
    family: SecurityFamily,
) -> CapabilityEntry {
    CapabilityEntry {
        language,
        family,
        algorithm: family.algorithm().to_string(),
        support: support(catalog, registry, language, family),
        nature: family.nature(),
        blocking: family.blocking(),
        limits: limits(language, family),
    }
}

fn support(
    catalog: &SemanticModelCatalog,
    registry: &BackendRegistry,
    language: SourceLanguage,
    family: SecurityFamily,
) -> Support {
    let unsupported = |reason: String| Support::Unsupported { reason };

    let adapter = catalog.adapter(language);
    if let Some(structural) = family.structural() {
        if !structural.supports(language) {
            return unsupported(format!(
                "{} has no model for {}",
                family.finding_kind(),
                language.name()
            ));
        }
    } else if adapter.is_none() {
        return unsupported(format!("no API model adapter for {}", language.name()));
    }

    // Reach is fixed by the shipped backend: intraprocedural.
    let reach = family.weakness().map(|_| AnalysisReach::Intraprocedural);
    let detector = family.detector(AnalysisReach::Intraprocedural);
    let backend = match registry.plan(&detector.requires) {
        Ok(backend) => backend,
        Err(plan) => {
            return unsupported(format!("no registered backend provides {:?}", plan.missing));
        }
    };
    let ceiling = backend.evidence_ceiling();
    let can_block = family
        .blocking()
        .gate()
        .is_some_and(|gate| ceiling.satisfies(gate.min_evidence_class));
    let models = family
        .weakness()
        .zip(adapter)
        .map(|(weakness, adapter)| ModelCounts {
            sources: adapter.models().filter(|m| m.is_source()).count(),
            sinks: adapter.models().filter(|m| m.sinks(weakness)).count(),
            sanitizers: adapter.models().filter(|m| m.sanitizes(weakness)).count(),
        });
    Support::Supported {
        backend: backend.name().to_string(),
        evidence_ceiling: ceiling,
        detector_version: PACK_VERSION.to_string(),
        logic_digest: detector.logic_digest().as_str().to_string(),
        reach,
        can_block,
        models,
    }
}

fn limits(language: SourceLanguage, family: SecurityFamily) -> Vec<String> {
    let mut limits = Vec::new();
    match family {
        SecurityFamily::SqlInjection => {
            limits
                .push("only the query-text argument is tracked; bound parameters are data".into());
        }
        SecurityFamily::CommandInjection => {
            limits.push(
                "only the program argument is modeled; a shell run with a tainted `-c` script is not"
                    .into(),
            );
        }
        SecurityFamily::PathTraversal => {
            limits.push(
                "only reducing a path to its last component sanitizes; canonicalize-and-prefix-check is not modeled and is reported as a flow"
                    .into(),
            );
        }
        SecurityFamily::HardcodedSecret => {
            limits.push(
                "name and literal heuristic only; entropy and values built across statements are not analysed"
                    .into(),
            );
        }
        SecurityFamily::UnsafeCode => {
            limits.push(
                "marks where unsafe is used; it does not judge whether the unsafe code is sound"
                    .into(),
            );
        }
        SecurityFamily::InfraPublicExposure => {
            limits.push(
                "public ACLs, world-open SSH/RDP/all-port ingress and public databases only; reachability through routing or load balancers is not analysed"
                    .into(),
            );
        }
        SecurityFamily::InfraEncryption => {
            limits.push(
                "storage-encryption flags of volumes, file systems and database instances only; provider defaults that changed across versions stay unknown"
                    .into(),
            );
        }
        SecurityFamily::InfraPermissions => {
            limits.push(
                "allow statements with a wildcard action on a wildcard resource only; conditions and partial wildcards are not judged"
                    .into(),
            );
        }
    }
    if matches!(
        family,
        SecurityFamily::InfraPublicExposure
            | SecurityFamily::InfraEncryption
            | SecurityFamily::InfraPermissions
    ) {
        limits.push(
            "configuration is read statically: a value is known only when the text fixes it, and any other value is unknown, never guessed. No plan, provider, credentials or module code is run"
                .into(),
        );
        limits.push(
            "only the modeled resource types are checked; remote modules are not opened; a negative result names what stayed unknown"
                .into(),
        );
    }
    if family.weakness().is_some() {
        limits.push("intraprocedural: flows across function calls are not followed".into());
        limits.push(
            "callees match by exact qualified name; an unmodeled call makes a negative result Unknown"
                .into(),
        );
        if language.is_jvm() {
            limits.push(
                "JVM context (imports, classpath supertypes) is optional input; without it only fully qualified callees resolve. No JVM or Detekt scanner is required"
                    .into(),
            );
        }
    }
    limits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::findings::AstBackend;

    fn registry_with_ast_only() -> BackendRegistry {
        let mut registry = BackendRegistry::new();
        registry.register(Box::new(AstBackend));
        registry
    }

    #[test]
    fn every_family_is_taint_xor_structural_and_names_its_kind() {
        for family in SecurityFamily::ALL {
            assert_ne!(family.weakness().is_some(), family.structural().is_some());
            assert!(!family.finding_kind().is_empty());
            assert_eq!(
                family.detector(AnalysisReach::Intraprocedural).validate(),
                Ok(())
            );
        }
        for weakness in Weakness::ALL {
            assert_eq!(
                SecurityFamily::of_weakness(weakness).weakness(),
                Some(weakness)
            );
        }
    }

    #[test]
    fn unsafe_is_a_risk_that_can_never_block_and_the_rest_are_vulnerabilities() {
        assert_eq!(SecurityFamily::UnsafeCode.nature(), FindingNature::Risk);
        assert_eq!(SecurityFamily::UnsafeCode.blocking().gate(), None);
        assert_eq!(SecurityFamily::UnsafeCode.cwe(), None);
        for family in SecurityFamily::ALL
            .into_iter()
            .filter(|f| *f != SecurityFamily::UnsafeCode)
        {
            assert_eq!(family.nature(), FindingNature::Vulnerability);
            assert!(family.blocking().gate().is_some());
        }
    }

    #[test]
    fn taint_families_declare_class_b_and_secrets_class_c() {
        let min = |f: SecurityFamily| f.blocking().gate().map(|g| g.min_evidence_class);
        assert_eq!(min(SecurityFamily::SqlInjection), Some(EvidenceClass::B));
        assert_eq!(min(SecurityFamily::HardcodedSecret), Some(EvidenceClass::C));
    }

    #[test]
    fn a_registry_without_dataflow_makes_taint_families_unsupported_in_the_matrix() {
        let matrix =
            CapabilityMatrix::derive(&SemanticModelCatalog::builtin(), &registry_with_ast_only());
        for family in [SecurityFamily::SqlInjection, SecurityFamily::PathTraversal] {
            let cell = matrix.get(SourceLanguage::Java, family).unwrap();
            assert!(
                matches!(cell.support, Support::Unsupported { .. }),
                "{family:?}"
            );
        }
        let secrets = matrix
            .get(SourceLanguage::Java, SecurityFamily::HardcodedSecret)
            .unwrap();
        assert!(matches!(secrets.support, Support::Supported { .. }));
    }

    #[test]
    fn matrix_has_one_cell_per_language_and_family() {
        let matrix =
            CapabilityMatrix::derive(&SemanticModelCatalog::builtin(), &registry_with_ast_only());
        assert_eq!(
            matrix.entries.len(),
            SourceLanguage::ALL.len() * SecurityFamily::ALL.len()
        );
        assert_eq!(matrix.pack_version, PACK_VERSION);
    }

    #[test]
    fn a_language_without_adapter_is_unsupported_for_every_family() {
        let matrix =
            CapabilityMatrix::derive(&SemanticModelCatalog::builtin(), &registry_with_ast_only());
        for family in SecurityFamily::ALL {
            let cell = matrix.get(SourceLanguage::Python, family).unwrap();
            assert!(
                matches!(cell.support, Support::Unsupported { .. }),
                "{family:?}"
            );
        }
    }
}
