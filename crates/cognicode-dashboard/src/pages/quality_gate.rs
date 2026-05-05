//! Quality gate page with conditions and status

use leptos::prelude::*;
use crate::state::{GateCondition, QualityGateResult};
use crate::components::{Shell, GateStatusBar};

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

#[component]
pub fn QualityGatePage() -> impl IntoView {
    let gate = mock_gate();

    view! {
        <Shell>
            <div style="max-width: 1200px; margin: 0 auto;">
                <header style="margin-bottom: 48px;">
                    <h1 class="text-h1">Quality Gate</h1>
                    <p style="margin-top: 8px; color: var(--color-text-secondary);">
                        Monitor your project quality gate status and conditions
                    </p>
                </header>

                <section style="margin-bottom: 48px;">
                    <GateStatusBar gate={gate.clone()} />
                </section>

                <section style="margin-bottom: 48px;">
                    <h2 class="text-h2" style="margin-bottom: 24px;">Gate Conditions</h2>
                    <div class="card" style="padding: 0; overflow: hidden;">
                        <table style="width: 100%; border-collapse: collapse;">
                            <thead>
                                <tr style="background: var(--color-surface-raised); border-bottom: 1px solid var(--color-border);">
                                    <th style="text-align: left; padding: 16px; font-size: 12px; font-weight: 600; color: var(--color-text-secondary); text-transform: uppercase; letter-spacing: 0.05em;">Status</th>
                                    <th style="text-align: left; padding: 16px; font-size: 12px; font-weight: 600; color: var(--color-text-secondary); text-transform: uppercase; letter-spacing: 0.05em;">Condition</th>
                                    <th style="text-align: left; padding: 16px; font-size: 12px; font-weight: 600; color: var(--color-text-secondary); text-transform: uppercase; letter-spacing: 0.05em;">Metric</th>
                                    <th style="text-align: right; padding: 16px; font-size: 12px; font-weight: 600; color: var(--color-text-secondary); text-transform: uppercase; letter-spacing: 0.05em;">Operator</th>
                                    <th style="text-align: right; padding: 16px; font-size: 12px; font-weight: 600; color: var(--color-text-secondary); text-transform: uppercase; letter-spacing: 0.05em;">Threshold</th>
                                </tr>
                            </thead>
                            <tbody>
                                {gate.conditions.iter().map(|condition| {
                                    view! {
                                        <ConditionRow condition={condition.clone()} />
                                    }
                                }).collect::<Vec<_>>()}
                            </tbody>
                        </table>
                    </div>
                </section>

                <section>
                    <h2 class="text-h2" style="margin-bottom: 24px;">Available Gates</h2>
                    <GateCards />
                </section>
            </div>
        </Shell>
    }
}

#[component]
fn ConditionRow(condition: GateCondition) -> impl IntoView {
    let (status_bg, status_color, status_icon) = if condition.passed {
        ("rgba(34, 197, 94, 0.1)", "#22c55e", "M5 13l4 4L19 7")
    } else {
        ("rgba(239, 68, 68, 0.1)", "#ef4444", "M6 18L18 6M6 6l12 12")
    };

    view! {
        <tr style="border-bottom: 1px solid var(--color-border);">
            <td style="padding: 16px;">
                <span style={format!("display: inline-flex; align-items: center; justify-content: center; width: 28px; height: 28px; border-radius: 50%; background: {};", status_bg)}>
                    <svg style={format!("width: 16px; height: 16px; color: {};", status_color)} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <path stroke-linecap="round" stroke-linejoin="round" d={status_icon}/>
                    </svg>
                </span>
            </td>
            <td style="padding: 16px; font-weight: 500;">{condition.name.clone()}</td>
            <td style="padding: 16px; font-family: monospace; font-size: 14px; color: var(--color-text-secondary);">{condition.metric.clone()}</td>
            <td style="padding: 16px; font-family: monospace; font-size: 14px; text-align: right;">{condition.operator.clone()}</td>
            <td style="padding: 16px; font-family: monospace; font-size: 14px; text-align: right; font-weight: 500;">{condition.threshold.to_string()}</td>
        </tr>
    }
}

#[component]
fn GateCards() -> impl IntoView {
    let available_gates = vec![
        ("SonarQube Way", "Default SonarQube quality gate", true),
        ("SonarQube Way - Strict", "Strict version with higher thresholds", false),
        ("Security Defaults", "Focused on security Hotspots", false),
        ("Production Prevents", "Strict gate for production releases", false),
    ];

    view! {
        <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 16px;">
            {available_gates.iter().map(|(name, description, is_active)| {
                let gate_name = name.to_string();
                let gate_desc = description.to_string();
                let border_style = if *is_active { "var(--color-brand)" } else { "transparent" };
                view! {
                    <div
                        class="card"
                        style={format!("cursor: pointer; transition: all 0.2s ease; border: 2px solid {};", border_style)}
                    >
                        <div style="display: flex; align-items: flex-start; justify-content: space-between; margin-bottom: 8px;">
                            <h3 style="margin: 0; font-size: 16px; font-weight: 600;">{gate_name}</h3>
                            <span style={format!("font-size: 11px; font-weight: 600; padding: 4px 8px; border-radius: 4px; background: {}; color: {}; text-transform: uppercase;",
                                if *is_active { "var(--color-brand)" } else { "transparent" },
                                if *is_active { "white" } else { "transparent" }
                            )}>
                                {if *is_active { "Active" } else { "" }}
                            </span>
                        </div>
                        <p style="margin: 0; font-size: 14px; color: var(--color-text-muted);">{gate_desc}</p>
                    </div>
                }
            }).collect::<Vec<_>>()}
        </div>
    }
}