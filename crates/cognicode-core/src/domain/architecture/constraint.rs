//! Architecture constraint model (WU1).
//!
//! Pure domain types: no I/O, no `sqlx`/`tokio`. Serialisable so that
//! constraints can be persisted by an adapter (out of scope for e77
//! first slice) without forcing a re-shape.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::domain::findings::namespaced::NamespacedName;

// ============================================================================
// Identifiers
// ============================================================================

/// Stable id of an architecture constraint. Same `namespace.name`
/// semantics as [`FindingKind`](crate::domain::findings::FindingKind) so
/// the id can be referenced from outside (ADR text, tests, manifests).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ArchitectureConstraintId(NamespacedName);

impl ArchitectureConstraintId {
    /// Build a new id. The name must already be namespaced
    /// (`a.b.c`-style); this method delegates the grammar validation
    /// to [`NamespacedName::new`].
    pub fn new(value: impl Into<String>) -> Result<Self, ConstraintError> {
        let name = NamespacedName::new(value).map_err(|_| ConstraintError::EmptyId)?;
        Ok(Self(name))
    }

    /// Borrow the string form.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for ArchitectureConstraintId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.as_str())
    }
}

// ============================================================================
// Layers
// ============================================================================

/// Hexagonal layer a module belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayerId {
    Domain,
    Application,
    Infrastructure,
    Bin,
    /// A module that does not map to a known layer (e.g. `tests/`,
    /// `benches/`, generated code). Constraints do not fire on these by
    /// default — they need an explicit override.
    Unknown,
}

impl LayerId {
    /// Best-effort resolution from a `cognicode-core` module path.
    ///
    /// Returns `Unknown` for anything outside `domain/`,
    /// `application/`, `infrastructure/`, and `bin.rs` / `bin/`. The
    /// evaluator will only produce findings for known layers.
    pub fn from_module_path(module_path: &str) -> Self {
        // We treat the crate-relative path as the canonical key. Tests
        // live under `tests/` and benches under `benches/`; both are
        // Unknown. The `lib.rs` / `mod.rs` / `bin.rs` style files are
        // resolved by their parent folder.
        let normalised = module_path.replace('\\', "/");
        // Strip a leading `crate::` if any.
        let normalised = normalised
            .strip_prefix("crate::")
            .unwrap_or(&normalised);
        // Drop everything past the first occurrence of these top-level
        // segments.
        for top in [
            "domain",
            "application",
            "infrastructure",
            "bin",
        ] {
            if let Some(rest) = normalised.strip_prefix(top) {
                // We have matched. The next character must be `::`,
                // `/`, or the end of string.
                let next = rest.chars().next();
                if matches!(next, Some(':') | Some('/') | None) {
                    return match top {
                        "domain" => LayerId::Domain,
                        "application" => LayerId::Application,
                        "infrastructure" => LayerId::Infrastructure,
                        "bin" => LayerId::Bin,
                        _ => unreachable!(),
                    };
                }
            }
        }
        LayerId::Unknown
    }
}

impl fmt::Display for LayerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Domain => "domain",
            Self::Application => "application",
            Self::Infrastructure => "infrastructure",
            Self::Bin => "bin",
            Self::Unknown => "unknown",
        })
    }
}

// ============================================================================
// Rule families
// ============================================================================

/// Rule 1 — a layer must not depend on a forbidden other layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerDependencyRule {
    /// The layer the rule protects (the "source").
    pub from_layer: LayerId,
    /// The layers the rule forbids reaching (the "forbidden targets").
    pub forbidden_targets: Vec<LayerId>,
    /// Human-readable explanation (e.g. "domain must not depend on
    /// infrastructure"). Not used by the evaluator; included for human
    /// consumers and for the EvidenceBundle.
    pub rationale: String,
}

/// Rule 2 — a layer must not import a given module path or crate name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForbiddenDependencyRule {
    /// The layer the rule protects.
    pub from_layer: LayerId,
    /// The forbidden module paths or crate names, as exact strings
    /// matched against the parsed `use` line.
    pub forbidden_paths: Vec<String>,
    /// Human-readable rationale.
    pub rationale: String,
}

/// Rule 3 — a namespace (a domain submodule) must not reach a forbidden
/// other namespace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamespaceBoundaryRule {
    /// The protected namespace, as a module path prefix (e.g.
    /// `domain::evidence_kernel`).
    pub caller_namespace: String,
    /// The forbidden target namespaces, as module path prefixes.
    pub forbidden_targets: Vec<String>,
    /// Human-readable rationale.
    pub rationale: String,
}

/// Concrete rule kind carried by a constraint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArchitectureConstraintKind {
    /// Rule 1 — `architecture.layer_dependency`.
    LayerDependency(LayerDependencyRule),
    /// Rule 2 — `architecture.forbidden_dependency`.
    ForbiddenDependency(ForbiddenDependencyRule),
    /// Rule 3 — `architecture.namespace_boundary`.
    NamespaceBoundary(NamespaceBoundaryRule),
}

impl ArchitectureConstraintKind {
    /// The canonical namespaced kind that an evaluator will emit when
    /// the rule fires. Kept on the type so the rule and its emitted
    /// kind cannot drift.
    pub fn finding_kind(&self) -> &'static str {
        match self {
            Self::LayerDependency(_) => "architecture.layer_dependency",
            Self::ForbiddenDependency(_) => "architecture.forbidden_dependency",
            Self::NamespaceBoundary(_) => "architecture.namespace_boundary",
        }
    }
}

// ============================================================================
// Candidate vs admitted
// ============================================================================

/// A proposed constraint, not yet admitted. May **not** produce
/// findings; the evaluator only consults admitted constraints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstraintCandidate {
    pub id: ArchitectureConstraintId,
    pub kind: ArchitectureConstraintKind,
    /// Optional ADR reference, e.g. `"ADR-046"`. A bare reference has
    /// **no authority** — admission is the only thing that confers
    /// authority.
    pub adr_ref: Option<String>,
    /// The entity that proposed the candidate (e.g.
    /// `"human:dev@example"`, `"ai:claude-haiku"`,
    /// `"plugin:detect-bad-imports"`). Recorded for audit, not for
    /// authority.
    pub proposed_by: String,
}

/// A constraint that has been admitted. Only admitted constraints
/// produce drift findings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchitectureConstraint {
    pub id: ArchitectureConstraintId,
    pub kind: ArchitectureConstraintKind,
    /// Optional ADR reference (informational; does not confer authority).
    pub adr_ref: Option<String>,
    /// When the constraint was admitted (opaque token; the application
    /// layer is the source of truth for the timestamp format).
    pub admitted_at: String,
    /// Who admitted it. See [`Admitter`].
    pub admitted_by: Admitter,
    /// Optional reference to the EvidenceBundle that justified the
    /// admission (e.g. a Trial run that confirmed the rule has at
    /// least one real consumer). Optional because not every admission
    /// requires prior evidence — but every admission is recorded.
    pub admission_evidence: Option<ArchitectureEvidence>,
}

/// Evidence that backed an admission. The shape is intentionally small:
/// the only thing that matters for e77 is *that* there is evidence,
/// not *what* it says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchitectureEvidence {
    /// Free-form description (e.g. "trial-2026-09-12 — 3 violations
    /// detected on synthetic fixture").
    pub description: String,
}

/// Role of the entity that admitted a constraint.
///
/// This is a *who*, not a *what*. Authority for admitting a constraint
/// is the same as authority for any other gating decision: only a
/// promoted admitter may admit a constraint that produces findings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Admitter {
    /// The admitter's stable id (e.g. `"human:dev@example"`,
    /// `"ci:job-42"`). No semantic meaning to the type.
    pub id: String,
    /// The role at admission time. Promoted admitter = may admit; any
    /// other role = may not.
    pub role: AdmitterRole,
}

/// Roles recognised by the admission flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdmitterRole {
    /// A human operator with promotion authority.
    HumanPromoter,
    /// An automated CI/CD pipeline with promotion authority (e.g. a
    /// signed release job).
    CiPromoter,
    /// Anything else. May propose constraints; may **not** admit them.
    Other,
}

impl Admitter {
    /// The only admitter roles that may turn a candidate into a
    /// constraint. `Other` is rejected by the admission flow
    /// ([`ConstraintAdmission::admit`]) and produces ZERO findings.
    pub fn may_admit(&self) -> bool {
        matches!(self.role, AdmitterRole::HumanPromoter | AdmitterRole::CiPromoter)
    }
}

/// Result of an admission attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstraintAdmission {
    pub constraint: ArchitectureConstraint,
    /// What happened.
    pub disposition: ConstraintAdmissionDisposition,
}

/// What the admission service decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintAdmissionDisposition {
    /// The candidate was admitted and may now produce findings.
    Admitted,
    /// The candidate was rejected. Common reasons: admitter not
    /// promoted, candidate id already admitted, malformed rule body.
    Rejected,
}

// ============================================================================
// Errors
// ============================================================================

/// Errors raised by the constraint model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstraintError {
    /// The id was empty or otherwise failed namespaced-name validation.
    EmptyId,
    /// The candidate id was malformed.
    MalformedCandidate,
    /// The candidate was rejected because the admitter is not
    /// promoted.
    AdmitterNotPromoted,
    /// A candidate with the same id was already admitted.
    AlreadyAdmitted,
    /// The rule body failed validation (e.g. empty forbidden_targets).
    InvalidRuleBody,
}

impl fmt::Display for ConstraintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::EmptyId => "empty or malformed constraint id",
            Self::MalformedCandidate => "malformed constraint candidate",
            Self::AdmitterNotPromoted => "admitter is not promoted",
            Self::AlreadyAdmitted => "constraint already admitted",
            Self::InvalidRuleBody => "invalid rule body",
        })
    }
}

impl std::error::Error for ConstraintError {}
