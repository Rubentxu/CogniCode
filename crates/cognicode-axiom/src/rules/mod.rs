//! Rule management — quality rules, code smells, security hotspots, and analysis pipeline
//!
//! Provides:
//! - **types**: Core Rule trait, Issue, Severity, Category, RuleContext with CallGraph helpers
//! - **catalog**: 18 built-in rules (code smells, vulnerabilities, security, bugs)
//! - **gates**: Quality Gate evaluation with YAML conditions
//! - **profiles**: Quality profile engine with YAML inheritance
//! - **debt**: SQALE technical debt calculation with A-E ratings
//! - **ratings**: Project-level reliability/security/maintainability ratings
//! - **duplication**: BLAKE3-based code duplication detection
//! - **store**: In-memory rule storage (legacy CRUD, kept for compatibility)
//! - **validator**: Rule validation (kept for compatibility)
//! - **adr_parser**: Architecture Decision Record parser

pub mod store;
pub mod validator;
pub mod adr_parser;
pub mod types;
pub mod catalog;
pub mod importer; // SonarQube rule importer
pub mod gates;      // Quality Gate System (Section 4)
pub mod profiles;    // Quality Profiles with YAML (Section 5)
pub mod debt;        // Technical Debt SQALE (Section 6)
pub mod ratings;     // Project Ratings A-E (Section 7)
pub mod duplication; // Duplication Detection with BLAKE3 (Section 3.1)

#[cfg(feature = "scraper")]
pub mod scraper;
#[cfg(feature = "scraper")]
pub use scraper::{SonarQubeScraper, scrape_command};

#[cfg(test)]
mod catalog_tests;

#[cfg(test)]
mod catalog_tests_generated;

pub use store::{RuleStore, RuleId};
pub use store::Rule as GovernanceRule;
pub use validator::RuleValidator;
pub use adr_parser::AdrParser;
pub use types::{
    RuleRegistry, Severity, Category, Issue, Remediation, RuleEntry,
    Rule, RuleContext, FileMetrics, ParseCache,
};
pub use gates::{QualityGate, QualityGateResult, GateCondition, CompareOperator, ProjectMetrics, QualityGateEvaluator, MetricValue};
pub use profiles::{QualityProfile, QualityProfileEngine, RuleConfig, ResolvedProfile};
pub use debt::{TechnicalDebtCalculator, TechnicalDebtReport, DebtRating, DebtCategory};
pub use ratings::{ProjectRatings, RatingDetails};
pub use duplication::{DuplicationDetector, DuplicationGroup, DuplicationLocation};
pub use importer::{ImportedRule, RuleCatalog, RuleParameter};

// Auto-discovered modules from build.rs
#[cfg(feature = "auto-discover")]
include!(concat!(env!("OUT_DIR"), "/rules_auto.rs"));
