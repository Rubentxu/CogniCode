//! Technical Debt (SQALE) Calculation
//!
//! Implements section 6 of doc 09: SQALE-based technical debt calculation
//! that maps issues to remediation efforts and computes debt ratios.

use std::collections::HashMap;

use crate::rules::types::{Category, Issue, Severity};

/// SQALE debt categories mapped to issue types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DebtCategory {
    Maintainability,
    Reliability,
    Security,
    Performance,
    Portability,
    Reusability,
    Efficiency,
    Documentation,
}

impl DebtCategory {
    /// Get the SQALE category label
    pub fn label(&self) -> &'static str {
        match self {
            DebtCategory::Maintainability => "Maintainability",
            DebtCategory::Reliability => "Reliability",
            DebtCategory::Security => "Security",
            DebtCategory::Performance => "Performance",
            DebtCategory::Portability => "Portability",
            DebtCategory::Reusability => "Reusability",
            DebtCategory::Efficiency => "Efficiency",
            DebtCategory::Documentation => "Documentation",
        }
    }

    /// Map issue category to debt category
    pub fn from_issue_category(category: Category) -> Self {
        match category {
            Category::Bug => DebtCategory::Reliability,
            Category::Vulnerability => DebtCategory::Security,
            Category::CodeSmell => DebtCategory::Maintainability,
            Category::SecurityHotspot => DebtCategory::Security,
        }
    }
}

/// Debt rating grades A-E
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum DebtRating {
    A = 5,  // Excellent - no debt
    B = 4,  // Good - minor debt
    C = 3,  // Satisfactory - moderate debt
    D = 2,  // Poor - significant debt
    E = 1,  // Critical - severe debt
}

impl DebtRating {
    /// Get the rating label
    pub fn label(&self) -> char {
        match self {
            DebtRating::A => 'A',
            DebtRating::B => 'B',
            DebtRating::C => 'C',
            DebtRating::D => 'D',
            DebtRating::E => 'E',
        }
    }

    /// Get description of this rating
    pub fn description(&self) -> &'static str {
        match self {
            DebtRating::A => "Excellent - No technical debt",
            DebtRating::B => "Good - Minor technical debt",
            DebtRating::C => "Satisfactory - Moderate technical debt",
            DebtRating::D => "Poor - Significant technical debt",
            DebtRating::E => "Critical - Severe technical debt",
        }
    }
}

/// Debt information for a single category
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CategoryDebt {
    /// Total debt in minutes
    pub debt_minutes: u64,
    /// Number of issues in this category
    pub issue_count: usize,
}

impl CategoryDebt {
    /// Create a new category debt report
    pub fn new() -> Self {
        Self {
            debt_minutes: 0,
            issue_count: 0,
        }
    }
}

impl Default for CategoryDebt {
    fn default() -> Self {
        Self::new()
    }
}

/// Complete technical debt report
#[derive(Debug, Clone, serde::Serialize)]
pub struct TechnicalDebtReport {
    /// Total debt in minutes
    pub total_debt_minutes: u64,
    /// Debt as a ratio (debt / total development time)
    pub debt_ratio: f64,
    /// Overall debt rating
    pub rating: DebtRating,
    /// Debt breakdown by category
    pub by_category: HashMap<String, CategoryDebt>,
    /// Total number of issues
    pub total_issues: usize,
    /// Total lines of code analyzed
    pub ncloc: usize,
}

impl Default for TechnicalDebtReport {
    fn default() -> Self {
        Self {
            total_debt_minutes: 0,
            debt_ratio: 0.0,
            rating: DebtRating::A,
            by_category: HashMap::new(),
            total_issues: 0,
            ncloc: 0,
        }
    }
}

/// SQALE remediation time multipliers (in minutes)
/// Based on SQALE method: https://www.sqale.org/
const REMEDIATION_TIMES: &[(Severity, u32)] = &[
    (Severity::Info, 5),       // 5 minutes for info
    (Severity::Minor, 15),     // 15 minutes for minor
    (Severity::Major, 60),    // 1 hour for major
    (Severity::Critical, 240), // 4 hours for critical
    (Severity::Blocker, 480), // 8 hours for blocker
];

/// Technical debt calculator using SQALE method
#[derive(Debug)]
pub struct TechnicalDebtCalculator;

impl TechnicalDebtCalculator {
    /// Create a new debt calculator
    pub fn new() -> Self {
        Self
    }

    /// Calculate technical debt from a list of issues
    pub fn calculate(&self, issues: &[Issue], ncloc: usize) -> TechnicalDebtReport {
        let mut by_category: HashMap<String, CategoryDebt> = HashMap::new();
        let mut total_debt_minutes: u64 = 0;

        for issue in issues {
            let debt_minutes = self.issue_debt_minutes(issue);
            let category_key = DebtCategory::from_issue_category(issue.category).label();

            total_debt_minutes += debt_minutes;

            let cat_debt = by_category.entry(category_key.to_string()).or_default();
            cat_debt.debt_minutes += debt_minutes;
            cat_debt.issue_count += 1;
        }

        // Calculate debt ratio
        // SQALE debt ratio = total debt / (ncloc * development time per line)
        // Assuming 30 minutes per line of code as baseline
        let development_time_minutes = (ncloc as f64) * 30.0;
        let debt_ratio = if development_time_minutes > 0.0 {
            (total_debt_minutes as f64) / development_time_minutes
        } else {
            0.0
        };

        // Calculate rating based on debt ratio thresholds
        let rating = Self::debt_ratio_to_rating(debt_ratio);

        TechnicalDebtReport {
            total_debt_minutes,
            debt_ratio,
            rating,
            by_category,
            total_issues: issues.len(),
            ncloc,
        }
    }

    /// Calculate debt in minutes for a single issue
    pub fn issue_debt_minutes(&self, issue: &Issue) -> u64 {
        // Base time from severity
        let base_time = REMEDIATION_TIMES
            .iter()
            .find(|(sev, _)| *sev == issue.severity)
            .map(|(_, time)| *time)
            .unwrap_or(15); // Default to minor if not found

        // Apply category multiplier
        let category_multiplier = match issue.category {
            Category::Bug => 2.0,           // Bugs cost more to fix
            Category::Vulnerability => 2.5, // Security issues are expensive
            Category::CodeSmell => 1.0,     // Standard
            Category::SecurityHotspot => 1.5,
        };

        // Multi-line issues take longer
        let line_factor = if let Some(end_line) = issue.end_line {
            let lines = end_line.saturating_sub(issue.line) + 1;
            (lines as f64).sqrt().max(1.0)
        } else {
            1.0
        };

        ((base_time as f64) * category_multiplier * line_factor) as u64
    }

    /// Convert debt ratio to a rating
    pub fn debt_ratio_to_rating(ratio: f64) -> DebtRating {
        // SQALE rating thresholds:
        // A: <= 5%  (0.05)
        // B: <= 10% (0.10)
        // C: <= 20% (0.20)
        // D: <= 50% (0.50)
        // E: > 50%  (>0.50)
        if ratio <= 0.05 {
            DebtRating::A
        } else if ratio <= 0.10 {
            DebtRating::B
        } else if ratio <= 0.20 {
            DebtRating::C
        } else if ratio <= 0.50 {
            DebtRating::D
        } else {
            DebtRating::E
        }
    }

    /// Format debt as human-readable string
    pub fn format_debt(&self, minutes: u64) -> String {
        if minutes < 60 {
            format!("{} minutes", minutes)
        } else if minutes < 480 {
            let hours = minutes / 60;
            let mins = minutes % 60;
            if mins == 0 {
                format!("{} hour{}", hours, if hours > 1 { "s" } else { "" })
            } else {
                format!("{}h {}m", hours, mins)
            }
        } else {
            let days = minutes / 480;
            let hours = (minutes % 480) / 60;
            if hours == 0 {
                format!("{} day{}", days, if days > 1 { "s" } else { "" })
            } else {
                format!("{}d {}h", days, hours)
            }
        }
    }
}

impl Default for TechnicalDebtCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_issue(severity: Severity, category: Category, line: usize) -> Issue {
        Issue::new(
            "TEST-RULE",
            "Test issue",
            severity,
            category,
            PathBuf::from("test.rs"),
            line,
        )
    }

    #[test]
    fn test_debt_calculation_empty() {
        let calculator = TechnicalDebtCalculator::new();
        let report = calculator.calculate(&[], 1000);
        
        assert_eq!(report.total_debt_minutes, 0);
        assert_eq!(report.debt_ratio, 0.0);
        assert_eq!(report.rating, DebtRating::A);
    }

    #[test]
    fn test_debt_calculation_single_issue() {
        let calculator = TechnicalDebtCalculator::new();
        let issue = create_test_issue(Severity::Major, Category::CodeSmell, 10);
        let report = calculator.calculate(&[issue], 1000);
        
        assert!(report.total_debt_minutes > 0);
        assert!(report.by_category.contains_key("Maintainability"));
    }

    #[test]
    fn test_debt_rating_thresholds() {
        assert_eq!(TechnicalDebtCalculator::debt_ratio_to_rating(0.03), DebtRating::A);
        assert_eq!(TechnicalDebtCalculator::debt_ratio_to_rating(0.05), DebtRating::A);
        assert_eq!(TechnicalDebtCalculator::debt_ratio_to_rating(0.07), DebtRating::B);
        assert_eq!(TechnicalDebtCalculator::debt_ratio_to_rating(0.15), DebtRating::C);
        assert_eq!(TechnicalDebtCalculator::debt_ratio_to_rating(0.35), DebtRating::D);
        assert_eq!(TechnicalDebtCalculator::debt_ratio_to_rating(0.60), DebtRating::E);
    }

    #[test]
    fn test_issue_debt_minutes() {
        let calc = TechnicalDebtCalculator::new();
        let issue = create_test_issue(Severity::Critical, Category::Bug, 50);
        
        let debt = calc.issue_debt_minutes(&issue);
        assert!(debt > 0);
    }

    #[test]
    fn test_multi_line_issue_increases_debt() {
        let calc = TechnicalDebtCalculator::new();
        
        let single_line = create_test_issue(Severity::Major, Category::CodeSmell, 10);
        let mut multi_line = single_line.clone();
        multi_line.end_line = Some(25);
        
        let single_debt = calc.issue_debt_minutes(&single_line);
        let multi_debt = calc.issue_debt_minutes(&multi_line);
        
        assert!(multi_debt > single_debt);
    }

    #[test]
    fn test_format_debt() {
        let calc = TechnicalDebtCalculator::new();
        
        assert_eq!(calc.format_debt(30), "30 minutes");
        assert_eq!(calc.format_debt(60), "1 hour");
        assert_eq!(calc.format_debt(90), "1h 30m");
        assert_eq!(calc.format_debt(480), "1 day");
        assert_eq!(calc.format_debt(960), "2 days");
        // 1200 min = 2 days + 240 min = 2d 4h
        assert_eq!(calc.format_debt(1200), "2d 4h");
    }

    #[test]
    fn test_category_mapping() {
        let bug = create_test_issue(Severity::Major, Category::Bug, 10);
        let vuln = create_test_issue(Severity::Major, Category::Vulnerability, 10);
        let smell = create_test_issue(Severity::Major, Category::CodeSmell, 10);
        
        let calc = TechnicalDebtCalculator::new();
        
        let bug_debt = calc.issue_debt_minutes(&bug);
        let vuln_debt = calc.issue_debt_minutes(&vuln);
        let smell_debt = calc.issue_debt_minutes(&smell);
        
        // Vulnerability should cost more than bug which costs more than code smell
        assert!(vuln_debt > bug_debt);
        assert!(bug_debt > smell_debt);
    }
}
