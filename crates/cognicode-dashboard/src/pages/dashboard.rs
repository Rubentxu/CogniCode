//! Dashboard page with full mock data

use leptos::prelude::*;
use crate::state::{
    Severity, Category, IssueResult, ProjectRatings,
    TechnicalDebt, GateCondition, QualityGateResult,
};
use crate::components::{
    Shell, RatingCard, MetricCard, GateStatusBar,
    Trend,
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
        label: "2h 45min".to_string(),
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
            GateCondition {
                id: "3".to_string(),
                name: "Maintainability Rating".to_string(),
                metric: "maintainability_rating".to_string(),
                operator: "<=".to_string(),
                threshold: 1.0,
                passed: true,
            },
            GateCondition {
                id: "4".to_string(),
                name: "Blocker Issues".to_string(),
                metric: "blocker_issues".to_string(),
                operator: "=".to_string(),
                threshold: 0.0,
                passed: true,
            },
            GateCondition {
                id: "5".to_string(),
                name: "Critical Issues".to_string(),
                metric: "critical_issues".to_string(),
                operator: "=".to_string(),
                threshold: 0.0,
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
            severity: Severity::Minor,
            category: Category::Maintainability,
            file: "src/main/java/com/example/Service.java".to_string(),
            line: 42,
            column: Some(13),
            end_line: Some(42),
            remediation_hint: Some("Consider using IllegalArgumentException or a custom exception".to_string()),
        },
        IssueResult {
            rule_id: "java:S1135".to_string(),
            message: "Complete the task implementation to avoid code smell.".to_string(),
            severity: Severity::Info,
            category: Category::Maintainability,
            file: "src/main/java/com/example/Controller.java".to_string(),
            line: 78,
            column: Some(5),
            end_line: Some(78),
            remediation_hint: None,
        },
        IssueResult {
            rule_id: "java:S3752".to_string(),
            message: "This URL should be parameterised to prevent SQL injection.".to_string(),
            severity: Severity::Major,
            category: Category::Security,
            file: "src/main/java/com/example/Repository.java".to_string(),
            line: 156,
            column: Some(20),
            end_line: Some(156),
            remediation_hint: Some("Use PreparedStatement or a framework that handles parameterisation".to_string()),
        },
        IssueResult {
            rule_id: "java:S2229".to_string(),
            message: "This class should be made 'final' or have a private constructor.".to_string(),
            severity: Severity::Minor,
            category: Category::Security,
            file: "src/main/java/com/example/AuthProvider.java".to_string(),
            line: 23,
            column: Some(14),
            end_line: Some(23),
            remediation_hint: None,
        },
        IssueResult {
            rule_id: "java:S1114".to_string(),
            message: "Remove this redundant null check, 'obj' is already guaranteed to be non-null at this point.".to_string(),
            severity: Severity::Major,
            category: Category::Reliability,
            file: "src/main/java/com/example/Processor.java".to_string(),
            line: 89,
            column: Some(9),
            end_line: Some(92),
            remediation_hint: None,
        },
    ]
}

#[component]
pub fn DashboardPage() -> impl IntoView {
    let ratings = mock_ratings();
    let debt = mock_debt();
    let gate = mock_gate();
    let issues = mock_issues();

    view! {
        <Shell>
            <div style="max-width: 1400px; margin: 0 auto;">
                <header style="margin-bottom: 48px;">
                    <h1 class="text-h1">Quality Dashboard</h1>
                    <p class="text-body text-text-secondary" style="margin-top: 8px;">Last analysis: 2 hours ago - 847 lines of code</p>
                </header>

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
                            trend={Some(Trend::down("15min"))}
                            icon={Some("M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z")}
                        />
                        <MetricCard
                            label="Issues Found"
                            value="47".to_string()
                            trend={Some(Trend::down("12%"))}
                            icon={Some("M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2")}
                        />
                        <MetricCard
                            label="Code Coverage"
                            value="73.2%".to_string()
                            trend={Some(Trend::up("3.1%"))}
                            icon={Some("M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z")}
                        />
                        <MetricCard
                            label="Duplicates"
                            value="2.4%".to_string()
                            trend={Some(Trend::neutral("0%"))}
                            icon={Some("M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z")}
                        />
                    </div>
                </section>

                <section style="margin-bottom: 48px;">
                    <h2 class="text-h2" style="margin-bottom: 24px;">Quality Gate</h2>
                    <GateStatusBar gate={gate} />
                </section>

                <section style="margin-bottom: 48px;">
                    <h2 class="text-h2" style="margin-bottom: 24px;">
                        Recent Issues
                        <span class="text-body-sm text-text-muted" style="margin-left: 8px;">({issues.len()} issues)</span>
                    </h2>
                    <div class="card">
                        <p class="text-body">{issues.len()} issues found</p>
                    </div>
                </section>
            </div>
        </Shell>
    }
}