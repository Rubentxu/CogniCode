//! Project Ratings A-E
//!
//! Implements section 7 of doc 09: Project-level ratings for reliability,
//! security, and maintainability derived from issue analysis.

use crate::rules::debt::{DebtRating, TechnicalDebtReport};
use crate::rules::types::{Category, Issue};

/// Project-level ratings combining multiple quality dimensions
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProjectRatings {
    /// Reliability rating (based on bugs)
    pub reliability: DebtRating,
    /// Security rating (based on vulnerabilities and security hotspots)
    pub security: DebtRating,
    /// Maintainability rating (based on technical debt ratio)
    pub maintainability: DebtRating,
}

impl ProjectRatings {
    /// Compute ratings from issues and debt report
    pub fn compute(issues: &[Issue], ncloc: usize, debt_report: &TechnicalDebtReport) -> Self {
        let reliability = Self::compute_reliability(issues, ncloc);
        let security = Self::compute_security(issues, ncloc);
        let maintainability = debt_report.rating;

        Self {
            reliability,
            security,
            maintainability,
        }
    }

    /// Compute reliability rating based on bug density
    fn compute_reliability(issues: &[Issue], ncloc: usize) -> DebtRating {
        let bugs: usize = issues
            .iter()
            .filter(|i| i.category == Category::Bug)
            .count();

        // Bug density = bugs per 1000 lines
        let density = if ncloc > 0 {
            (bugs as f64) / (ncloc as f64) * 1000.0
        } else {
            0.0
        };

        // Rating thresholds based on bug density
        // A: 0 bugs per 1000 lines
        // B: <= 0.5 bugs per 1000 lines
        // C: <= 1.0 bugs per 1000 lines
        // D: <= 2.0 bugs per 1000 lines
        // E: > 2.0 bugs per 1000 lines
        if density == 0.0 {
            DebtRating::A
        } else if density <= 0.5 {
            DebtRating::B
        } else if density <= 1.0 {
            DebtRating::C
        } else if density <= 2.0 {
            DebtRating::D
        } else {
            DebtRating::E
        }
    }

    /// Compute security rating based on vulnerabilities and security hotspots
    fn compute_security(issues: &[Issue], ncloc: usize) -> DebtRating {
        let vulnerabilities: usize = issues
            .iter()
            .filter(|i| i.category == Category::Vulnerability)
            .count();

        let hotspots: usize = issues
            .iter()
            .filter(|i| i.category == Category::SecurityHotspot)
            .count();

        // Combined security issue density
        let total_security_issues = vulnerabilities + hotspots;
        let density = if ncloc > 0 {
            (total_security_issues as f64) / (ncloc as f64) * 1000.0
        } else {
            0.0
        };

        // Security rating is more strict:
        // A: 0 security issues per 1000 lines
        // B: <= 0.1 per 1000 lines
        // C: <= 0.5 per 1000 lines
        // D: <= 1.0 per 1000 lines
        // E: > 1.0 per 1000 lines
        if density == 0.0 {
            DebtRating::A
        } else if density <= 0.1 {
            DebtRating::B
        } else if density <= 0.5 {
            DebtRating::C
        } else if density <= 1.0 {
            DebtRating::D
        } else {
            DebtRating::E
        }
    }

    /// Get the overall project rating (worst of the three)
    pub fn overall(&self) -> char {
        // Overall rating is the minimum (worst) of the three
        let min_rating = self.reliability
            .min(self.security)
            .min(self.maintainability);
        min_rating.label()
    }

    /// Get the overall rating as a DebtRating enum
    pub fn overall_rating(&self) -> DebtRating {
        self.reliability
            .min(self.security)
            .min(self.maintainability)
    }

    /// Check if the project meets a minimum rating threshold
    /// A rating of 'A' is best (5), 'E' is worst (1)
    pub fn meets_threshold(&self, threshold: char) -> bool {
        let overall_rating = self.overall_rating();
        // Convert char to DebtRating
        let threshold_rating = match threshold {
            'A' => DebtRating::A,
            'B' => DebtRating::B,
            'C' => DebtRating::C,
            'D' => DebtRating::D,
            'E' => DebtRating::E,
            _ => return false,
        };
        // Compare ordinals: higher is better, so we need overall >= threshold
        overall_rating >= threshold_rating
    }

    /// Get a summary description
    pub fn summary(&self) -> String {
        format!(
            "Project Rating: {} (Reliability: {}, Security: {}, Maintainability: {})",
            self.overall(),
            self.reliability.label(),
            self.security.label(),
            self.maintainability.label()
        )
    }

    /// Get detailed rating information
    pub fn details(&self) -> RatingDetails {
        RatingDetails {
            reliability: self.reliability,
            security: self.security,
            maintainability: self.maintainability,
            overall: self.overall(),
        }
    }
}

/// Detailed rating breakdown
#[derive(Debug, Clone)]
pub struct RatingDetails {
    pub reliability: DebtRating,
    pub security: DebtRating,
    pub maintainability: DebtRating,
    pub overall: char,
}

impl Default for ProjectRatings {
    fn default() -> Self {
        Self {
            reliability: DebtRating::A,
            security: DebtRating::A,
            maintainability: DebtRating::A,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Severity;
    use std::path::PathBuf;

    fn create_issue(category: Category, severity: Severity, line: usize) -> Issue {
        Issue::new(
            "TEST",
            "Test issue",
            severity,
            category,
            PathBuf::from("test.rs"),
            line,
        )
    }

    #[test]
    fn test_all_excellent() {
        let issues = vec![];
        let debt_report = TechnicalDebtReport {
            total_debt_minutes: 0,
            debt_ratio: 0.0,
            rating: DebtRating::A,
            by_category: HashMap::new(),
            total_issues: 0,
            ncloc: 1000,
        };

        let ratings = ProjectRatings::compute(&issues, 1000, &debt_report);
        
        assert_eq!(ratings.reliability, DebtRating::A);
        assert_eq!(ratings.security, DebtRating::A);
        assert_eq!(ratings.maintainability, DebtRating::A);
        assert_eq!(ratings.overall(), 'A');
    }

    #[test]
    fn test_reliability_rating() {
        let debt_report = TechnicalDebtReport {
            total_debt_minutes: 0,
            debt_ratio: 0.0,
            rating: DebtRating::A,
            by_category: HashMap::new(),
            total_issues: 0,
            ncloc: 1000,
        };

        // 0 bugs -> A
        let ratings = ProjectRatings::compute(&[], 1000, &debt_report);
        assert_eq!(ratings.reliability, DebtRating::A);

        // Create 1 bug per 1000 lines -> C
        let bugs = vec![create_issue(Category::Bug, Severity::Major, 1)];
        let ratings = ProjectRatings::compute(&bugs, 1000, &debt_report);
        assert_eq!(ratings.reliability, DebtRating::C);
    }

    #[test]
    fn test_security_rating() {
        let debt_report = TechnicalDebtReport {
            total_debt_minutes: 0,
            debt_ratio: 0.0,
            rating: DebtRating::A,
            by_category: HashMap::new(),
            total_issues: 0,
            ncloc: 1000,
        };

        // 0 security issues -> A
        let ratings = ProjectRatings::compute(&[], 1000, &debt_report);
        assert_eq!(ratings.security, DebtRating::A);

        // 2 vulnerabilities per 1000 lines -> E
        let vulns = vec![
            create_issue(Category::Vulnerability, Severity::Critical, 1),
            create_issue(Category::Vulnerability, Severity::Major, 2),
        ];
        let ratings = ProjectRatings::compute(&vulns, 1000, &debt_report);
        assert_eq!(ratings.security, DebtRating::E);
    }

    #[test]
    fn test_overall_rating_is_worst() {
        let debt_report = TechnicalDebtReport {
            total_debt_minutes: 0,
            debt_ratio: 0.0,
            rating: DebtRating::D,  // Maintainability is D
            by_category: HashMap::new(),
            total_issues: 0,
            ncloc: 1000,
        };

        let issues = vec![
            create_issue(Category::Bug, Severity::Major, 1),
        ];

        let ratings = ProjectRatings::compute(&issues, 1000, &debt_report);
        
        // Overall should be D (worst of A, A, D)
        assert_eq!(ratings.overall(), 'D');
    }

    #[test]
    fn test_meets_threshold() {
        let ratings = ProjectRatings {
            reliability: DebtRating::B,
            security: DebtRating::C,
            maintainability: DebtRating::A,
        };

        // Overall rating is C (minimum of B, C, A)
        assert!(!ratings.meets_threshold('A')); // C < A
        assert!(!ratings.meets_threshold('B')); // C < B
        assert!(ratings.meets_threshold('C')); // C >= C
        assert!(ratings.meets_threshold('D')); // C >= D
        assert!(ratings.meets_threshold('E')); // C >= E
    }

    use std::collections::HashMap;
}
