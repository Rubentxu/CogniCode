//! Dashboard page with analysis overview

use leptos::prelude::*;
use crate::state::{
    IssueResult, ProjectRatings, TechnicalDebt, GateCondition, QualityGateResult,
};
use crate::components::{
    Shell, RatingCard, MetricCard, GateStatusBar, IssueTable,
};

fn mock_ratings() -> ProjectRatings {
    ProjectRatings {
        reliability: 'A',
        security: 'B',
        maintainability: 'B',
        coverage: 'C',
    }
}

fn mock_debt() -> TechnicalDebt {
    TechnicalDebt {
        total_minutes: 245,
        rating: 'C',
        label: "4h 5min".to_string(),
    }
}

fn mock_gate() -> QualityGateResult {
    QualityGateResult {
        name: "SonarQube Way".to_string(),
        status: "PASSED".to_string(),
        conditions: vec![
            GateCondition {
                id: "1".to_string(),
                name: "Reliability Rating".to_string(),
                metric: "reliability_rating".to_string(),
                operator: "<=".to_string(),
                threshold: 1.0,
                passed: true,
            },
            GateCondition {
                id: "2".to_string(),
                name: "Security Rating".to_string(),
                metric: "security_rating".to_string(),
                operator: "<=".to_string(),
                threshold: 2.0,
                passed: true,
            },
        ],
    }
}

fn mock_issues() -> Vec<IssueResult> {
    vec![
        IssueResult {
            rule_id: "java:S1130".to_string(),
            message: "Replace this generic exception declaration with a more specific one.".to_string(),
            severity: crate::state::Severity::Minor,
            category: crate::state::Category::Maintainability,
            file: "src/main/java/com/example/Service.java".to_string(),
            line: 42,
            column: Some(13),
            end_line: Some(42),
            remediation_hint: Some("Consider using IllegalArgumentException".to_string()),
        },
        IssueResult {
            rule_id: "java:S3752".to_string(),
            message: "This URL should be parameterised to prevent SQL injection.".to_string(),
            severity: crate::state::Severity::Major,
            category: crate::state::Category::Security,
            file: "src/main/java/com/example/Repository.java".to_string(),
            line: 156,
            column: Some(20),
            end_line: Some(156),
            remediation_hint: Some("Use PreparedStatement".to_string()),
        },
    ]
}

#[component]
pub fn DashboardPage() -> impl IntoView {
    let ratings = mock_ratings();
    let debt = mock_debt();
    let gate = mock_gate();
    let issues = mock_issues();
    let recent_issues: Vec<IssueResult> = issues.iter().take(5).cloned().collect();

    view! {
        <Shell>
            <div style="max-width: 1400px; margin: 0 auto;">
                <header style="margin-bottom: 48px;">
                    <h1 class="text-h1">Quality Dashboard</h1>
                    <p class="text-body text-text-secondary" style="margin-top: 8px;">
                        Last analysis: Just now - 847 lines of code
                    </p>
                </header>

                <section style="margin-bottom: 48px;">
                    <GateStatusBar gate={gate} />
                </section>

                <section style="margin-bottom: 48px;">
                    <h2 class="text-h2" style="margin-bottom: 24px;">Project Ratings</h2>
                    <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 24px;">
                        <RatingCard rating={ratings.reliability} label="Reliability" />
                        <RatingCard rating={ratings.security} label="Security" />
                        <RatingCard rating={ratings.maintainability} label="Maintainability" />
                        <RatingCard rating={ratings.coverage} label="Coverage" />
                    </div>
                </section>

                <section style="margin-bottom: 48px;">
                    <h2 class="text-h2" style="margin-bottom: 24px;">Key Metrics</h2>
                    <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 24px;">
                        <MetricCard
                            label="Technical Debt"
                            value={debt.label.clone()}
                            trend={None}
                            icon={None}
                        />
                        <MetricCard
                            label="Issues Found"
                            value="50".to_string()
                            trend={None}
                            icon={None}
                        />
                        <MetricCard
                            label="Lines of Code"
                            value="847".to_string()
                            trend={None}
                            icon={None}
                        />
                        <MetricCard
                            label="Blocker Issues"
                            value="0".to_string()
                            trend={None}
                            icon={None}
                        />
                    </div>
                </section>

                <section>
                    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 24px;">
                        <h2 class="text-h2">
                            Recent Issues
                        </h2>
                        <a href="/issues" style="font-size: 14px; color: var(--color-text-link); text-decoration: none; display: inline-flex; align-items: center; gap: 4px;">
                            View all 50 issues
                            <svg style="width: 16px; height: 16px;" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7"/>
                            </svg>
                        </a>
                    </div>
                    <IssueTable issues={recent_issues} />
                </section>
            </div>
        </Shell>
    }
}