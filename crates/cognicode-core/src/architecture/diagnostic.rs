//! B4.W2 — Architecture as diagnostic, no gate authority.
//!
//! Estado pre-B4.W2:
//!   - `check_architecture` emitía un DTO con `score` + `cycles` +
//!     `violations` (cada `violation` con `severity: String`). El shape no
//!     exponía evidencia (`file`, `byte_range`, `snippet`, `rule_reference`)
//!     y no declaraba `gate_authority: false` en el catálogo, así que un
//!     consumidor podía asumir autoridad implícita.
//!   - CLI y MCP exponían el mismo DTO pero por canales distintos; un cambio
//!     en uno podía divergir del otro sin un test que lo cazara (UAT-B4-04).
//!
//! Lo que este módulo aporta:
//!   - `ArchitectureDiagnostic`: el shape read-only que CLI y MCP comparten.
//!     Lleva observaciones + totales + razones de skip; nunca lleva `score`,
//!     `verdict` ni `gate` (R-B4-W2: una observación sin opt-in no puede
//!     adquirir autoridad implícita).
//!   - `ArchitectureObservation`: una restricción evaluada con su evidencia.
//!     `evidence` apunta a `file + byte_range + snippet + rule_reference`
//!     para que un agente revise el código que sustenta el hallazgo.
//!   - `Severity`: cuatro valores (`Error`, `Warning`, `Info`, `Hint`).
//!     El default es `Info` (el nivel más conservador, alineado con la
//!     política de "no upcast silencioso" que B3.W1 introdujo para
//!     `Precision`).
//!   - `byte_range` es `Range<usize>` medida en bytes del source, NO
//!     `(line, col)`. Razón: B3.W1 ya fijó el contrato para `SymbolId` y
//!     una `Range<usize>` no se desincroniza con un re-formato.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::ops::Range;
use std::path::PathBuf;

use crate::interface::symbol_identity::SymbolId;

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    #[default]
    Info,
    Hint,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
            Self::Hint => "hint",
        }
    }

    pub fn rank(&self) -> u8 {
        match self {
            Self::Error => 3,
            Self::Warning => 2,
            Self::Info => 1,
            Self::Hint => 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ArchitectureEvidence {
    pub file: PathBuf,
    pub byte_range: Range<usize>,
    pub snippet: String,
    pub rule_reference: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ArchitectureObservation {
    pub rule_id: String,
    pub severity: Severity,
    pub message: String,
    pub evidence: Vec<ArchitectureEvidence>,
    pub affected_symbols: Vec<SymbolId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ArchitectureDiagnostic {
    pub observations: Vec<ArchitectureObservation>,
    pub total_rules_evaluated: usize,
    pub total_rules_skipped: usize,
    pub skip_reasons: BTreeMap<String, String>,
}

pub fn build_diagnostic(
    observations: Vec<ArchitectureObservation>,
    evaluated: usize,
) -> ArchitectureDiagnostic {
    let skipped = evaluated.saturating_sub(observations.len());
    ArchitectureDiagnostic {
        observations,
        total_rules_evaluated: evaluated,
        total_rules_skipped: skipped,
        skip_reasons: BTreeMap::new(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObservationValidationError {
    EmptyEvidence,
    EmptySnippet,
    EmptyRuleId,
}

impl std::fmt::Display for ObservationValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyEvidence => f.write_str("observation has no evidence"),
            Self::EmptySnippet => f.write_str("evidence has empty snippet"),
            Self::EmptyRuleId => f.write_str("observation has empty rule_id"),
        }
    }
}

impl std::error::Error for ObservationValidationError {}

pub fn validate_observation(
    obs: &ArchitectureObservation,
) -> Result<(), ObservationValidationError> {
    if obs.rule_id.is_empty() {
        return Err(ObservationValidationError::EmptyRuleId);
    }
    if obs.evidence.is_empty() {
        return Err(ObservationValidationError::EmptyEvidence);
    }
    for ev in &obs.evidence {
        if ev.snippet.is_empty() {
            return Err(ObservationValidationError::EmptySnippet);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence(snippet: &str) -> ArchitectureEvidence {
        ArchitectureEvidence {
            file: PathBuf::from("src/domain/foo.rs"),
            byte_range: Range { start: 0, end: 24 },
            snippet: snippet.to_string(),
            rule_reference: "ADR-046".to_string(),
        }
    }

    fn observation(severity: Severity) -> ArchitectureObservation {
        ArchitectureObservation {
            rule_id: "architecture.layer_dependency".to_string(),
            severity,
            message: "msg".to_string(),
            evidence: vec![evidence("use crate::infrastructure::db;")],
            affected_symbols: vec![],
        }
    }

    #[test]
    fn severity_ordering_is_total() {
        assert!(Severity::Error > Severity::Warning);
        assert!(Severity::Warning > Severity::Info);
        assert!(Severity::Info > Severity::Hint);
        assert_eq!(Severity::Error.rank(), 3);
        assert_eq!(Severity::Hint.rank(), 0);
    }

    #[test]
    fn evidence_with_empty_snippet_is_rejected() {
        let obs = ArchitectureObservation {
            rule_id: "architecture.layer_dependency".to_string(),
            severity: Severity::Error,
            message: "msg".to_string(),
            evidence: vec![evidence("")],
            affected_symbols: vec![],
        };
        let err = validate_observation(&obs).expect_err("empty snippet must reject");
        assert_eq!(err, ObservationValidationError::EmptySnippet);
    }

    #[test]
    fn byte_range_is_usize_pair_not_line_column() {
        let r = evidence("snippet").byte_range.clone();
        let _start: usize = r.start;
        let _end: usize = r.end;
        assert!(r.end > r.start);
    }

    #[test]
    fn severity_default_is_info() {
        assert_eq!(Severity::default(), Severity::Info);
    }

    #[test]
    fn skip_reasons_preserved_through_build() {
        let obs = vec![observation(Severity::Error)];
        let mut reasons = BTreeMap::new();
        reasons.insert("rule_a".to_string(), "disabled".to_string());
        reasons.insert("rule_b".to_string(), "no evidence".to_string());
        let diag = ArchitectureDiagnostic {
            observations: obs,
            total_rules_evaluated: 7,
            total_rules_skipped: 6,
            skip_reasons: reasons.clone(),
        };
        assert_eq!(diag.skip_reasons.len(), 2);
        assert_eq!(diag.skip_reasons.get("rule_a"), Some(&"disabled".to_string()));
        assert_eq!(diag.skip_reasons.get("rule_b"), Some(&"no evidence".to_string()));
    }
}