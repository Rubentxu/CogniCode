//! Code quality analysis
//!
//! SOLID principle heuristics, connascence metrics, LCOM (Lack of Cohesion of Methods),
//! boundary checking, and quality delta comparisons — all powered by the cognicode-core CallGraph.

pub mod lcom;
pub mod connascence;
pub mod solid;
pub mod delta;
pub mod boundary;

// Re-exports for public API
pub use lcom::{CohesionLevel, LcomCalculator, LcomResult};
pub use connascence::{
    ConnascenceAnalyzer, ConnascenceReport, ConnascenceThresholds,
    ConnascenceType, ConnascenceViolation,
};
pub use solid::{
    SolidChecker, SolidPrinciple, SolidReport, SolidScores, SolidViolation,
    TypeSolidReport,
};
pub use delta::{
    ComplexitySummary, ConnascenceReportSnapshot, QualityChange, QualityDelta,
    QualitySnapshot, SolidScoresSnapshot,
};
pub use boundary::{BoundaryChecker, BoundaryDefinition, BoundaryReport, BoundaryViolation};

/// Compute maintainability index using simplified SQALE formula
/// Formula: MI = max(0, min(100, 100 - 0.25*duplications - 0.25*complexity - 50*(lines/1000)))
pub fn maintainability_index(complexity: f64, duplications_pct: f64, ncloc: usize) -> f64 {
    let a = 100.0;
    let b = 0.25 * duplications_pct;
    let c = 0.25 * complexity;
    let d = 50.0 * (ncloc as f64 / 1000.0);
    (a - b - c - d).max(0.0).min(100.0)
}

/// Quality report that aggregates all quality metrics
#[derive(Debug, Clone, serde::Serialize)]
pub struct QualityReport {
    pub issues: Vec<crate::rules::types::Issue>,
    pub total_issues: usize,
    pub maintainability_index: f64,
    pub rating: char,
    pub debt_minutes: u64,
    pub code_smells: usize,
    pub bugs: usize,
    pub vulnerabilities: usize,
    pub duplications_pct: f64,
}

impl QualityReport {
    pub fn new(
        issues: Vec<crate::rules::types::Issue>,
        ncloc: usize,
        duplications_pct: f64,
    ) -> Self {
        let code_smells = issues.iter().filter(|i| matches!(i.category, crate::rules::Category::CodeSmell)).count();
        let bugs = issues.iter().filter(|i| matches!(i.category, crate::rules::Category::Bug)).count();
        let vulnerabilities = issues.iter().filter(|i| matches!(i.category, crate::rules::Category::Vulnerability)).count();

        let debt = crate::rules::TechnicalDebtCalculator::new().calculate(&issues, ncloc);
        let mi = maintainability_index(15.0, duplications_pct, ncloc);

        let rating = match debt.rating {
            crate::rules::DebtRating::A => 'A',
            crate::rules::DebtRating::B => 'B',
            crate::rules::DebtRating::C => 'C',
            crate::rules::DebtRating::D => 'D',
            crate::rules::DebtRating::E => 'E',
        };

        Self {
            total_issues: issues.len(),
            issues,
            maintainability_index: (mi * 100.0).round() / 100.0,
            rating,
            debt_minutes: debt.total_debt_minutes,
            code_smells,
            bugs,
            vulnerabilities,
            duplications_pct,
        }
    }
}