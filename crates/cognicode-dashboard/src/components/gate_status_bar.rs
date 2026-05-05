//! Gate status bar component with PASSED/FAILED styling

use leptos::prelude::*;
use crate::state::QualityGateResult;

#[component]
pub fn GateStatusBar(gate: QualityGateResult) -> impl IntoView {
    let is_passed = gate.status == "PASSED";
    let icon = if is_passed {
        "M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"
    } else {
        "M10 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2m7-2a9 9 0 11-18 0 9 9 0 0118 0z"
    };

    let (bg_color, border_color) = if is_passed {
        ("rgba(34, 197, 94, 0.1)", "#22c55e")
    } else {
        ("rgba(239, 68, 68, 0.1)", "#ef4444")
    };

    let passed_count = gate.conditions.iter().filter(|c| c.passed).count();
    let total_count = gate.conditions.len();

    view! {
        <div style={format!("border-radius: 24px; padding: 32px; box-shadow: var(--shadow-card); background: linear-gradient(135deg, {} 0%, {} 100%); border: 2px solid {};", bg_color, bg_color, border_color)}>
            <div style="display: flex; align-items: center; gap: 24px; margin-bottom: 24px;">
                <div style={format!("width: 56px; height: 56px; border-radius: 50%; display: flex; align-items: center; justify-content: center; flex-shrink: 0; background: {}; color: {};", bg_color, border_color)}>
                    <svg style="width: 32px; height: 32px;" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <path stroke-linecap="round" stroke-linejoin="round" d={icon}/>
                    </svg>
                </div>
                <div style="flex: 1;">
                    <h3 style={format!("margin: 0; font-size: 20px; font-weight: 700; color: {};", border_color)}>{gate.name.clone()}</h3>
                    <p style="margin: 4px 0 0 0; font-size: 16px; font-weight: 600;">
                        {gate.status.clone()}
                        <span style="font-weight: 400; color: var(--color-text-muted); font-size: 14px;">
                            {format!(" ({}/{} conditions passed)", passed_count, total_count)}
                        </span>
                    </p>
                </div>
            </div>
            <div style="display: flex; flex-direction: column; gap: 8px;">
                {gate.conditions.iter().map(|condition| {
                    let (cond_bg, cond_color) = if condition.passed {
                        ("rgba(34, 197, 94, 0.1)", "#22c55e")
                    } else {
                        ("rgba(239, 68, 68, 0.1)", "#ef4444")
                    };
                    view! {
                        <div style={format!("display: flex; justify-content: space-between; align-items: center; padding: 12px 16px; border-radius: 8px; font-size: 14px; background: {}; color: {};", cond_bg, cond_color)}>
                            <span style="font-weight: 500;">{condition.name.clone()}</span>
                            <span class="text-mono" style="font-size: 12px; opacity: 0.9;">
                                {format!("{} {} {}", condition.metric.clone(), condition.operator.clone(), condition.threshold)}
                            </span>
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </div>
    }
}