//! B4.W3 — Normalized quality finding representation.
//!
//! Estado pre-B4.W3: los 5 tools de calidad (`get_complexity`,
//! `detect_long_parameter_lists`, `detect_god_functions`, `solid_audit`
//! y análisis de relaciones) emitían DTOs no normalizados. Cada tool
//! tenía su propio shape (`ComplexityMetrics`, `LongParamsResult`,
//! `GodFunctionsResult`, `SolidReport`, ...). No existía un contrato
//! común que dijera "esto es un quality finding, con categoría +
//! severidad + confianza + fuente + location + regla + procedencia".
//!
//! Riesgo que este módulo cierra: un agente que lee los 5 outputs no
//! podía cruzar resultados por categoría, ni filtrar por confianza, ni
//! agregar por lenguaje. Y peor — al colapsar shapes heterogéneos
//! bajo el nombre "quality finding", un consumer podía presentar un
//! análisis de Rust como si fuera comparable byte-a-byte con un
//! análisis de Python. R-B4-W3: NO declarar equivalentes análisis
//! de lenguajes con capacidades diferentes.
//!
//! Política B4.W3:
//!   - Cada `Finding` lleva `category`, `severity`, `confidence`,
//!     `provenance` (tool + rule + algorithm + language) y
//!     `location` (per-file byte range, no project-wide offset).
//!   - `FindingsReport` agrega `findings`, `language_breakdown`,
//!     `truncated` y `total_files_scanned`. La suma de
//!     `language_breakdown` debe coincidir con `findings.len()`.
//!   - El catálogo `product/tools.json` declara `language_scope`
//!     para cada uno de los 5 tools: esa lista es el contrato
//!     observable de "qué lenguajes son analizables".

use std::collections::BTreeMap;
use std::ops::Range;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::interface::symbol_identity::SymbolId;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingCategory {
    Complexity,
    LongParameterList,
    GodFunction,
    SolidViolation,
    RelationshipCycle,
    Other(String),
}

impl FindingCategory {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Complexity => "complexity",
            Self::LongParameterList => "long_parameter_list",
            Self::GodFunction => "god_function",
            Self::SolidViolation => "solid_violation",
            Self::RelationshipCycle => "relationship_cycle",
            Self::Other(name) => name.as_str(),
        }
    }
}

impl std::fmt::Display for FindingCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for FindingSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FindingSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FindingProvenance {
    pub tool: String,
    pub rule: String,
    pub algorithm: String,
    pub language: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FindingLocation {
    pub file: PathBuf,
    pub byte_range: Range<usize>,
    pub function: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub category: FindingCategory,
    pub severity: FindingSeverity,
    pub message: String,
    pub location: FindingLocation,
    pub provenance: FindingProvenance,
    pub related_symbols: Vec<SymbolId>,
}

impl Finding {
    pub fn new(
        id: impl Into<String>,
        category: FindingCategory,
        severity: FindingSeverity,
        message: impl Into<String>,
        location: FindingLocation,
        provenance: FindingProvenance,
    ) -> Result<Self, FindingBuildError> {
        let id = id.into();
        let message = message.into();
        if id.trim().is_empty() {
            return Err(FindingBuildError::EmptyId);
        }
        if message.trim().is_empty() {
            return Err(FindingBuildError::EmptyMessage);
        }
        if !FindingsReport::confidence_in_range(provenance.confidence) {
            return Err(FindingBuildError::ConfidenceOutOfRange(
                provenance.confidence,
            ));
        }
        Ok(Self {
            id,
            category,
            severity,
            message,
            location,
            provenance,
            related_symbols: Vec::new(),
        })
    }

    pub fn with_related_symbols(mut self, symbols: Vec<SymbolId>) -> Self {
        self.related_symbols = symbols;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FindingBuildError {
    EmptyId,
    EmptyMessage,
    ConfidenceOutOfRange(f32),
}

impl std::fmt::Display for FindingBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyId => f.write_str("finding id must not be empty"),
            Self::EmptyMessage => f.write_str("finding message must not be empty"),
            Self::ConfidenceOutOfRange(c) => {
                write!(f, "confidence {c} out of [0.0, 1.0]")
            }
        }
    }
}

impl std::error::Error for FindingBuildError {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FindingsReport {
    pub findings: Vec<Finding>,
    pub total_files_scanned: usize,
    pub language_breakdown: BTreeMap<String, usize>,
    pub truncated: bool,
}

impl FindingsReport {
    pub fn from_findings(findings: Vec<Finding>, total_files_scanned: usize) -> Self {
        let language_breakdown = compute_language_breakdown(&findings);
        Self {
            findings,
            total_files_scanned,
            language_breakdown,
            truncated: false,
        }
    }

    pub fn with_truncation(
        findings: Vec<Finding>,
        total_files_scanned: usize,
        truncated: bool,
    ) -> Self {
        let language_breakdown = compute_language_breakdown(&findings);
        Self {
            findings,
            total_files_scanned,
            language_breakdown,
            truncated,
        }
    }

    pub fn confidence_in_range(c: f32) -> bool {
        (0.0..=1.0).contains(&c)
    }

    pub fn len(&self) -> usize {
        self.findings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.findings.is_empty()
    }
}

fn compute_language_breakdown(findings: &[Finding]) -> BTreeMap<String, usize> {
    let mut out: BTreeMap<String, usize> = BTreeMap::new();
    for f in findings {
        *out.entry(f.provenance.language.clone()).or_insert(0) += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(category: FindingCategory) -> Finding {
        let loc = FindingLocation {
            file: PathBuf::from("src/sample.rs"),
            byte_range: 10..20,
            function: Some("target".to_string()),
        };
        let prov = FindingProvenance {
            tool: "test-tool".to_string(),
            rule: "test-rule".to_string(),
            algorithm: "static-walk".to_string(),
            language: "rust".to_string(),
            confidence: 0.7,
        };
        Finding::new(
            "f-1",
            category,
            FindingSeverity::Medium,
            "test message",
            loc,
            prov,
        )
        .expect("valid sample finding")
    }

    #[test]
    fn finding_category_serialize_round_trip() {
        let cats = [
            FindingCategory::Complexity,
            FindingCategory::LongParameterList,
            FindingCategory::GodFunction,
            FindingCategory::SolidViolation,
            FindingCategory::RelationshipCycle,
            FindingCategory::Other("custom".to_string()),
        ];
        for c in cats {
            let json = serde_json::to_string(&c).unwrap();
            let back: FindingCategory = serde_json::from_str(&json).unwrap();
            assert_eq!(back, c);
        }
    }

    #[test]
    fn finding_severity_order_total() {
        use std::cmp::Ordering;
        let sevs = [
            FindingSeverity::Info,
            FindingSeverity::Low,
            FindingSeverity::Medium,
            FindingSeverity::High,
            FindingSeverity::Critical,
        ];
        for (i, a) in sevs.iter().enumerate() {
            for (j, b) in sevs.iter().enumerate() {
                let ord = a.cmp(b);
                if i < j {
                    assert_eq!(ord, Ordering::Less);
                } else if i > j {
                    assert_eq!(ord, Ordering::Greater);
                } else {
                    assert_eq!(ord, Ordering::Equal);
                }
            }
        }
    }

    #[test]
    fn finding_confidence_range_enforced() {
        assert!(FindingsReport::confidence_in_range(0.0));
        assert!(FindingsReport::confidence_in_range(1.0));
        assert!(!FindingsReport::confidence_in_range(-0.1));
        assert!(!FindingsReport::confidence_in_range(1.1));
    }

    #[test]
    fn finding_location_byte_range_is_per_file() {
        let a = FindingLocation {
            file: PathBuf::from("a.rs"),
            byte_range: 0..10,
            function: None,
        };
        let b = FindingLocation {
            file: PathBuf::from("b.rs"),
            byte_range: 0..10,
            function: None,
        };
        assert_eq!(a.byte_range, b.byte_range);
        assert_ne!(a.file, b.file);
    }

    #[test]
    fn report_language_breakdown_sums_to_total_findings() {
        let findings = vec![
            sample(FindingCategory::Complexity),
            sample(FindingCategory::GodFunction),
        ];
        let report = FindingsReport::from_findings(findings, 1);
        let sum: usize = report.language_breakdown.values().sum();
        assert_eq!(sum, report.findings.len());
    }

    #[test]
    fn finding_related_symbols_default_empty() {
        let f = sample(FindingCategory::Complexity);
        assert!(f.related_symbols.is_empty());
    }
}
