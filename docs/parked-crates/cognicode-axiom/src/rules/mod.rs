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
//! - **regex_patterns**: Shared regex pattern constants for rules

pub mod store;
pub mod validator;
pub mod adr_parser;
pub mod types;
pub mod catalog;
pub mod helpers;   // Shared helper functions for rules
pub mod rules;     // Extracted rules from catalog.rs (S138, S3776, S2306, S1066, S1192)
pub mod importer; // SonarQube rule importer
pub mod gates;      // Quality Gate System (Section 4)
pub mod profiles;    // Quality Profiles with YAML (Section 5)
pub mod debt;        // Technical Debt SQALE (Section 6)
pub mod ratings;     // Project Ratings A-E (Section 7)
pub mod duplication; // Duplication Detection with BLAKE3 (Section 3.1)
pub mod regex_patterns; // Shared regex pattern constants
pub mod subscription_engine; // Deterministic SubscriptionVisitor pattern (SonarQube-aligned)
pub mod preflight; // Layer-0 preflight filter using Aho-Corasick
pub mod poc_rules; // PoC rules using #[cogni_rule] attribute macro
pub mod symbol_table; // Lightweight per-file SymbolTable for LCPG MVP
pub mod visitor; // Reusable AST visitor trait and traversal patterns
// pub mod kb_security; // KB-generated security rules (auto-generated) — directory missing

#[cfg(feature = "scraper")]
pub mod scraper;
#[cfg(feature = "scraper")]
pub use scraper::{SonarQubeScraper, scrape_command};

#[cfg(test)]
mod catalog_tests;

#[cfg(test)]
mod catalog_tests_generated;

#[cfg(test)]
mod cogni_rule_tests;

pub use store::{RuleStore, RuleId};
pub use store::Rule as GovernanceRule;
pub use validator::RuleValidator;
pub use adr_parser::AdrParser;
pub use types::{
    RuleRegistry, Severity, Category, Issue, Remediation, RuleEntry,
    Rule, RuleContext, FileMetrics, ParseCache,
    CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity,
    EntityType, Scope,
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
