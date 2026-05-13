//! Diagram Comparison Page — Compare two diagrams side-by-side
//!
//! Two independent panels (A and B) each generate a workspace_json via format=json.
//! When both are ready, the diff endpoint is called with both workspace_jsons.

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;
use crate::state::ReactiveAppState;
use crate::components::{Shell, LoadingSpinner};
use crate::api::diagrams::*;
use crate::components::diagram_viewer::DiagramViewer;

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for a single diagram panel.
#[derive(Clone, Debug)]
pub struct DiagramConfig {
    pub project_path: String,
    pub diagram_type: DiagramType,
    pub c4_level: C4Level,
    pub entry_symbol: String,
}

impl Default for DiagramConfig {
    fn default() -> Self {
        Self {
            project_path: String::from("/home/rubentxu/Proyectos/rust/CogniCode-diagram-f5"),
            diagram_type: DiagramType::C4,
            c4_level: C4Level::Context,
            entry_symbol: String::new(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// DiagramDiffPage — main page component
// ─────────────────────────────────────────────────────────────────────────────

#[component]
pub fn DiagramDiffPage() -> impl IntoView {
    let _state = expect_context::<ReactiveAppState>();

    // ── Panel A state ─────────────────────────────────────────────────────────
    let (config_a, set_config_a) = signal(DiagramConfig::default());
    let (workspace_json_a, set_workspace_json_a) = signal(Option::<String>::None);
    let (generating_a, set_generating_a) = signal(false);

    // ── Panel B state ─────────────────────────────────────────────────────────
    let (config_b, set_config_b) = signal(DiagramConfig::default());
    let (workspace_json_b, set_workspace_json_b) = signal(Option::<String>::None);
    let (generating_b, set_generating_b) = signal(false);

    // ── Shared diff state ─────────────────────────────────────────────────────
    let (diff_response, set_diff_response) = signal(Option::<DiffDiagramResponse>::None);
    let (diffing, set_diffing) = signal(false);
    let (error_msg, set_error) = signal(Option::<String>::None);

    // ── Derived: can diff ─────────────────────────────────────────────────────
    let can_diff = move || workspace_json_a.get().is_some() && workspace_json_b.get().is_some();

    // ── Generate for panel A ───────────────────────────────────────────────────
    let generate_a = {
        move || {
            let cfg = config_a.get();
            set_generating_a.set(true);
            set_error.set(None);

            spawn_local(async move {
                let request = GenerateDiagramRequest {
                    project_path: cfg.project_path.clone(),
                    diagram_type: cfg.diagram_type.as_str().to_string(),
                    level: if cfg.diagram_type == DiagramType::C4 {
                        Some(cfg.c4_level.as_str().to_string())
                    } else {
                        None
                    },
                    entry_symbol: if cfg.diagram_type != DiagramType::C4
                        && cfg.diagram_type != DiagramType::MultiLang
                        && !cfg.entry_symbol.is_empty()
                    {
                        Some(cfg.entry_symbol.clone())
                    } else {
                        None
                    },
                    format: Some("json".to_string()),
                };

                match generate_diagram(request).await {
                    Ok(resp) => {
                        set_workspace_json_a.set(resp.workspace_json);
                    }
                    Err(e) => {
                        set_error.set(Some(e));
                    }
                }
                set_generating_a.set(false);
            });
        }
    };

    // ── Generate for panel B ───────────────────────────────────────────────────
    let generate_b = {
        move || {
            let cfg = config_b.get();
            set_generating_b.set(true);
            set_error.set(None);

            spawn_local(async move {
                let request = GenerateDiagramRequest {
                    project_path: cfg.project_path.clone(),
                    diagram_type: cfg.diagram_type.as_str().to_string(),
                    level: if cfg.diagram_type == DiagramType::C4 {
                        Some(cfg.c4_level.as_str().to_string())
                    } else {
                        None
                    },
                    entry_symbol: if cfg.diagram_type != DiagramType::C4
                        && cfg.diagram_type != DiagramType::MultiLang
                        && !cfg.entry_symbol.is_empty()
                    {
                        Some(cfg.entry_symbol.clone())
                    } else {
                        None
                    },
                    format: Some("json".to_string()),
                };

                match generate_diagram(request).await {
                    Ok(resp) => {
                        set_workspace_json_b.set(resp.workspace_json);
                    }
                    Err(e) => {
                        set_error.set(Some(e));
                    }
                }
                set_generating_b.set(false);
            });
        }
    };

    // ── Compute diff when both workspaces are ready ────────────────────────────
    let compute_diff = {
        move || {
            let json_a = match workspace_json_a.get() {
                Some(v) => v,
                None => return,
            };
            let json_b = match workspace_json_b.get() {
                Some(v) => v,
                None => return,
            };

            set_diffing.set(true);
            set_error.set(None);

            spawn_local(async move {
                let request = DiffDiagramRequest {
                    workspace_a_json: json_a,
                    workspace_b_json: json_b,
                    format: Some("json".to_string()),
                };

                match diff_diagrams(request).await {
                    Ok(resp) => set_diff_response.set(Some(resp)),
                    Err(e) => set_error.set(Some(e)),
                }
                set_diffing.set(false);
            });
        }
    };

    // Auto-trigger diff when both workspaces become available
    Effect::new(move |_| {
        let _ = workspace_json_a.get();
        let _ = workspace_json_b.get();
        if can_diff() {
            compute_diff();
        }
    });

    view! {
        <Shell>
            <div style="max-width: 1400px;">
                {/* Header */}
                <header style="margin-bottom: 32px;">
                    <h1 style="font-size: 28px; font-weight: 700; color: var(--color-text-primary); margin-bottom: 8px;">
                        "Diagram Comparison"
                    </h1>
                    <p style="font-size: 15px; color: var(--color-text-secondary);">
                        "Compare two versions of a diagram side-by-side. Generate each panel independently, then the diff runs automatically."
                    </p>
                </header>

                {/* Error Display */}
                <Show when={move || error_msg.get().is_some()} fallback={|| view! { <></> }}>
                    <div style="background: var(--color-accent-sunset); border-radius: 8px; padding: 16px; margin-bottom: 24px;">
                        <p style="color: var(--color-text-primary); font-size: 14px;">{error_msg.get().unwrap()}</p>
                    </div>
                </Show>

                {/* Two Panel Grid */}
                <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 24px; margin-bottom: 24px;">
                    <PanelACard
                        config_a={config_a}
                        set_config_a={set_config_a}
                        generating_a={generating_a}
                        workspace_json_a={workspace_json_a}
                        generate_a={generate_a}
                    />
                    <PanelBCard
                        config_b={config_b}
                        set_config_b={set_config_b}
                        generating_b={generating_b}
                        workspace_json_b={workspace_json_b}
                        generate_b={generate_b}
                    />
                </div>

                {/* Diffing indicator */}
                <Show when={move || diffing.get()} fallback={|| view! { <></> }}>
                    <div style="background: var(--color-surface-raised); border: 1px solid var(--color-border); border-radius: 12px; padding: 32px; text-align: center; margin-bottom: 24px;">
                        <LoadingSpinner message="Computing diff..." />
                    </div>
                </Show>

                {/* Diff Output */}
                <Show when={move || !diffing.get() && diff_response.get().is_some()} fallback={|| view! { <></> }}>
                    <DiffResultView response={diff_response.get().unwrap()} />
                </Show>

                {/* Empty State */}
                <Show when={move || !diffing.get() && diff_response.get().is_none() && workspace_json_a.get().is_none() && workspace_json_b.get().is_none()} fallback={|| view! { <></> }}>
                    <div style="background: var(--color-surface-raised); border: 1px solid var(--color-border); border-radius: 12px; padding: 48px; text-align: center;">
                        <svg style="width: 48px; height: 48px; color: var(--color-text-muted); margin: 0 auto 16px;" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M9 17V7m0 10a2 2 0 01-2 2H5a2 2 0 01-2-2V7a2 2 0 012-2h2a2 2 0 012 2m0 10a2 2 0 002 2h2a2 2 0 002-2M9 7a2 2 0 012-2h2a2 2 0 012 2m0 10V7m0 10a2 2 0 002 2h2a2 2 0 002-2V7a2 2 0 00-2-2h-2a2 2 0 00-2 2"/>
                        </svg>
                        <p style="color: var(--color-text-muted); font-size: 14px;">"Generate both panels to see the comparison"</p>
                    </div>
                </Show>

                {/* Waiting for both panels */}
                <Show when={move || !diffing.get() && diff_response.get().is_none() && (workspace_json_a.get().is_some() ^ workspace_json_b.get().is_some())} fallback={|| view! { <></> }}>
                    <div style="background: var(--color-surface-raised); border: 1px solid var(--color-border); border-radius: 12px; padding: 32px; text-align: center; margin-bottom: 24px;">
                        <div style="display: flex; align-items: center; justify-content: center; gap: 12px; margin-bottom: 12px;">
                            <svg style="width: 24px; height: 24px; color: var(--color-text-muted);" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M12 6v6h4.5m4.5 0a9 9 0 11-18 0 9 9 0 0118 0z"/>
                            </svg>
                            <p style="color: var(--color-text-secondary); font-size: 14px; font-weight: 500;">"Waiting for the other panel..."</p>
                        </div>
                        <p style="color: var(--color-text-muted); font-size: 13px;">"Generate both diagrams to trigger the comparison automatically"</p>
                    </div>
                </Show>
            </div>
        </Shell>
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Sub-components
// ─────────────────────────────────────────────────────────────────────────────

/// Panel A card.
#[component]
fn PanelACard(
    config_a: ReadSignal<DiagramConfig>,
    set_config_a: WriteSignal<DiagramConfig>,
    generating_a: ReadSignal<bool>,
    workspace_json_a: ReadSignal<Option<String>>,
    generate_a: impl Fn() + Clone + 'static,
) -> impl IntoView {
    let show_level = move || config_a.get().diagram_type == DiagramType::C4;
    let show_entry = move || {
        let dt = config_a.get().diagram_type;
        dt != DiagramType::C4 && dt != DiagramType::MultiLang
    };

    view! {
        <div style="background: var(--color-surface-raised); border: 1px solid var(--color-border); border-radius: 12px; padding: 20px; box-shadow: var(--shadow-card);">
            <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 20px;">
                <div style="display: flex; align-items: center; gap: 10px;">
                    <span style={format!("padding: 4px 12px; font-size: 12px; font-weight: 700; color: #fff; background: {}; border-radius: 6px;", if workspace_json_a.get().is_some() { "#16a34a" } else { "var(--color-text-secondary)" })}>"PANEL A"</span>
                    <Show when={move || generating_a.get()} fallback={|| view! { <></> }}>
                        <span style="font-size: 12px; color: var(--color-text-muted);">"Generating..."</span>
                    </Show>
                </div>
            </div>

            <div style="display: flex; flex-direction: column; gap: 16px;">
                {/* Project Path */}
                <div>
                    <label style="display: block; font-size: 13px; font-weight: 500; color: var(--color-text-secondary); margin-bottom: 6px;">"Project Path"</label>
                    <input type="text" value={config_a.get().project_path.clone()}
                        on:input=move |ev| { set_config_a.update(|c| c.project_path = event_target_value(&ev)); }
                        placeholder="/path/to/project"
                        style="width: 100%; padding: 10px 14px; font-size: 14px; font-family: monospace; color: var(--color-text-primary); background: var(--color-surface); border: 1px solid var(--color-border); border-radius: 8px; outline: none; box-sizing: border-box;" />
                </div>

                {/* Diagram Type */}
                <div>
                    <label style="display: block; font-size: 13px; font-weight: 500; color: var(--color-text-secondary); margin-bottom: 6px;">"Diagram Type"</label>
                    <select on:change=move |ev| {
                        let val = event_target_value(&ev);
                        let dt = match val.as_str() {
                            "c4" => DiagramType::C4,
                            "sequence" => DiagramType::Sequence,
                            "state_machine" => DiagramType::StateMachine,
                            "activity" => DiagramType::Activity,
                            "multi_lang" => DiagramType::MultiLang,
                            _ => DiagramType::C4,
                        };
                        set_config_a.update(|c| c.diagram_type = dt);
                    } style="width: 100%; padding: 10px 14px; font-size: 14px; color: var(--color-text-primary); background: var(--color-surface); border: 1px solid var(--color-border); border-radius: 8px; outline: none; cursor: pointer; box-sizing: border-box;">
                        {DiagramType::all().iter().map(|dt| {
                            let label = dt.label();
                            let value = dt.as_str();
                            let selected = *dt == config_a.get().diagram_type;
                            view! { <option value={value} selected={selected}>{label}</option> }
                        }).collect::<Vec<_>>()}
                    </select>
                </div>

                {/* C4 Level */}
                <Show when={show_level} fallback={|| view! { <></> }}>
                    <div>
                        <label style="display: block; font-size: 13px; font-weight: 500; color: var(--color-text-secondary); margin-bottom: 6px;">"C4 Level"</label>
                        <select on:change=move |ev| {
                            let val = event_target_value(&ev);
                            let level = match val.as_str() {
                                "context" => C4Level::Context,
                                "container" => C4Level::Container,
                                "component" => C4Level::Component,
                                "code" => C4Level::Code,
                                _ => C4Level::Context,
                            };
                            set_config_a.update(|c| c.c4_level = level);
                        } style="width: 100%; padding: 10px 14px; font-size: 14px; color: var(--color-text-primary); background: var(--color-surface); border: 1px solid var(--color-border); border-radius: 8px; outline: none; cursor: pointer; box-sizing: border-box;">
                            {C4Level::all().iter().map(|lvl| {
                                let label = lvl.label();
                                let value = lvl.as_str();
                                let selected = *lvl == config_a.get().c4_level;
                                view! { <option value={value} selected={selected}>{label}</option> }
                            }).collect::<Vec<_>>()}
                        </select>
                    </div>
                </Show>

                {/* Entry Symbol */}
                <Show when={show_entry} fallback={|| view! { <></> }}>
                    <div>
                        <label style="display: block; font-size: 13px; font-weight: 500; color: var(--color-text-secondary); margin-bottom: 6px;">"Entry Symbol"</label>
                        <input type="text" value={config_a.get().entry_symbol.clone()}
                            on:input=move |ev| { set_config_a.update(|c| c.entry_symbol = event_target_value(&ev)); }
                            placeholder="e.g., main, MyStruct, handle_request"
                            style="width: 100%; padding: 10px 14px; font-size: 14px; font-family: monospace; color: var(--color-text-primary); background: var(--color-surface); border: 1px solid var(--color-border); border-radius: 8px; outline: none; box-sizing: border-box;" />
                    </div>
                </Show>

                {/* Workspace Status */}
                <Show when={move || workspace_json_a.get().is_some()} fallback={|| view! { <></> }}>
                    <div style="display: flex; align-items: center; gap: 8px; padding: 10px 14px; background: rgba(34, 197, 94, 0.12); border: 1px solid rgba(34, 197, 94, 0.3); border-radius: 8px; font-size: 13px; color: #16a34a;">
                        <svg style="width: 16px; height: 16px; flex-shrink: 0;" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M9 12.75L11.25 15 15 9.75M21 12a9 9 0 11-18 0 9 9 0 0118 0z"/>
                        </svg>
                        "Workspace generated"
                    </div>
                </Show>

                {/* Generate Button */}
                <button on:click=move |_| generate_a() disabled={generating_a.get()}
                    style="width: 100%; padding: 12px 20px; font-size: 14px; font-weight: 600; color: #ffffff; background: var(--color-accent-sky); border: none; border-radius: 8px; cursor: pointer; transition: all 0.15s ease; opacity: if generating_a.get() { 0.7 } else { 1.0 };">
                    {move || if generating_a.get() { "Generating A..." } else { "Generate A" }}
                </button>
            </div>
        </div>
    }
}

/// Panel B card.
#[component]
fn PanelBCard(
    config_b: ReadSignal<DiagramConfig>,
    set_config_b: WriteSignal<DiagramConfig>,
    generating_b: ReadSignal<bool>,
    workspace_json_b: ReadSignal<Option<String>>,
    generate_b: impl Fn() + Clone + 'static,
) -> impl IntoView {
    let show_level = move || config_b.get().diagram_type == DiagramType::C4;
    let show_entry = move || {
        let dt = config_b.get().diagram_type;
        dt != DiagramType::C4 && dt != DiagramType::MultiLang
    };

    view! {
        <div style="background: var(--color-surface-raised); border: 1px solid var(--color-border); border-radius: 12px; padding: 20px; box-shadow: var(--shadow-card);">
            <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 20px;">
                <div style="display: flex; align-items: center; gap: 10px;">
                    <span style={format!("padding: 4px 12px; font-size: 12px; font-weight: 700; color: #fff; background: {}; border-radius: 6px;", if workspace_json_b.get().is_some() { "#16a34a" } else { "var(--color-text-secondary)" })}>"PANEL B"</span>
                    <Show when={move || generating_b.get()} fallback={|| view! { <></> }}>
                        <span style="font-size: 12px; color: var(--color-text-muted);">"Generating..."</span>
                    </Show>
                </div>
            </div>

            <div style="display: flex; flex-direction: column; gap: 16px;">
                {/* Project Path */}
                <div>
                    <label style="display: block; font-size: 13px; font-weight: 500; color: var(--color-text-secondary); margin-bottom: 6px;">"Project Path"</label>
                    <input type="text" value={config_b.get().project_path.clone()}
                        on:input=move |ev| { set_config_b.update(|c| c.project_path = event_target_value(&ev)); }
                        placeholder="/path/to/project"
                        style="width: 100%; padding: 10px 14px; font-size: 14px; font-family: monospace; color: var(--color-text-primary); background: var(--color-surface); border: 1px solid var(--color-border); border-radius: 8px; outline: none; box-sizing: border-box;" />
                </div>

                {/* Diagram Type */}
                <div>
                    <label style="display: block; font-size: 13px; font-weight: 500; color: var(--color-text-secondary); margin-bottom: 6px;">"Diagram Type"</label>
                    <select on:change=move |ev| {
                        let val = event_target_value(&ev);
                        let dt = match val.as_str() {
                            "c4" => DiagramType::C4,
                            "sequence" => DiagramType::Sequence,
                            "state_machine" => DiagramType::StateMachine,
                            "activity" => DiagramType::Activity,
                            "multi_lang" => DiagramType::MultiLang,
                            _ => DiagramType::C4,
                        };
                        set_config_b.update(|c| c.diagram_type = dt);
                    } style="width: 100%; padding: 10px 14px; font-size: 14px; color: var(--color-text-primary); background: var(--color-surface); border: 1px solid var(--color-border); border-radius: 8px; outline: none; cursor: pointer; box-sizing: border-box;">
                        {DiagramType::all().iter().map(|dt| {
                            let label = dt.label();
                            let value = dt.as_str();
                            let selected = *dt == config_b.get().diagram_type;
                            view! { <option value={value} selected={selected}>{label}</option> }
                        }).collect::<Vec<_>>()}
                    </select>
                </div>

                {/* C4 Level */}
                <Show when={show_level} fallback={|| view! { <></> }}>
                    <div>
                        <label style="display: block; font-size: 13px; font-weight: 500; color: var(--color-text-secondary); margin-bottom: 6px;">"C4 Level"</label>
                        <select on:change=move |ev| {
                            let val = event_target_value(&ev);
                            let level = match val.as_str() {
                                "context" => C4Level::Context,
                                "container" => C4Level::Container,
                                "component" => C4Level::Component,
                                "code" => C4Level::Code,
                                _ => C4Level::Context,
                            };
                            set_config_b.update(|c| c.c4_level = level);
                        } style="width: 100%; padding: 10px 14px; font-size: 14px; color: var(--color-text-primary); background: var(--color-surface); border: 1px solid var(--color-border); border-radius: 8px; outline: none; cursor: pointer; box-sizing: border-box;">
                            {C4Level::all().iter().map(|lvl| {
                                let label = lvl.label();
                                let value = lvl.as_str();
                                let selected = *lvl == config_b.get().c4_level;
                                view! { <option value={value} selected={selected}>{label}</option> }
                            }).collect::<Vec<_>>()}
                        </select>
                    </div>
                </Show>

                {/* Entry Symbol */}
                <Show when={show_entry} fallback={|| view! { <></> }}>
                    <div>
                        <label style="display: block; font-size: 13px; font-weight: 500; color: var(--color-text-secondary); margin-bottom: 6px;">"Entry Symbol"</label>
                        <input type="text" value={config_b.get().entry_symbol.clone()}
                            on:input=move |ev| { set_config_b.update(|c| c.entry_symbol = event_target_value(&ev)); }
                            placeholder="e.g., main, MyStruct, handle_request"
                            style="width: 100%; padding: 10px 14px; font-size: 14px; font-family: monospace; color: var(--color-text-primary); background: var(--color-surface); border: 1px solid var(--color-border); border-radius: 8px; outline: none; box-sizing: border-box;" />
                    </div>
                </Show>

                {/* Workspace Status */}
                <Show when={move || workspace_json_b.get().is_some()} fallback={|| view! { <></> }}>
                    <div style="display: flex; align-items: center; gap: 8px; padding: 10px 14px; background: rgba(34, 197, 94, 0.12); border: 1px solid rgba(34, 197, 94, 0.3); border-radius: 8px; font-size: 13px; color: #16a34a;">
                        <svg style="width: 16px; height: 16px; flex-shrink: 0;" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M9 12.75L11.25 15 15 9.75M21 12a9 9 0 11-18 0 9 9 0 0118 0z"/>
                        </svg>
                        "Workspace generated"
                    </div>
                </Show>

                {/* Generate Button */}
                <button on:click=move |_| generate_b() disabled={generating_b.get()}
                    style="width: 100%; padding: 12px 20px; font-size: 14px; font-weight: 600; color: #ffffff; background: var(--color-accent-sky); border: none; border-radius: 8px; cursor: pointer; transition: all 0.15s ease; opacity: if generating_b.get() { 0.7 } else { 1.0 };">
                    {move || if generating_b.get() { "Generating B..." } else { "Generate B" }}
                </button>
            </div>
        </div>
    }
}

/// Renders the diff result: summary stats + diff_output.
#[component]
fn DiffResultView(response: DiffDiagramResponse) -> impl IntoView {
    let summary = response.summary.clone();
    let containers_added = response.containers_added.clone();
    let containers_removed = response.containers_removed.clone();
    let containers_modified = response.containers_modified.clone();
    let diff_output = response.diff_output.clone();

    // Extract all values upfront to avoid moving fields
    let summary_containers_added = summary.containers_added;
    let summary_containers_removed = summary.containers_removed;
    let summary_containers_modified = summary.containers_modified;
    let summary_total_changes = summary.total_changes;
    let rel_added = response.relationships_added_count;
    let rel_removed = response.relationships_removed_count;

    // Extract booleans for Show conditions
    let has_added = !containers_added.is_empty();
    let has_removed = !containers_removed.is_empty();
    let has_modified = !containers_modified.is_empty();

    view! {
        <div style="display: flex; flex-direction: column; gap: 24px;">

            {/* Summary Stats */}
            <div style="background: var(--color-surface-raised); border: 1px solid var(--color-border); border-radius: 12px; padding: 24px; box-shadow: var(--shadow-card);">
                <h2 style="font-size: 16px; font-weight: 600; color: var(--color-text-primary); margin-bottom: 16px;">"Diff Summary"</h2>

                <div style="display: flex; gap: 16px; flex-wrap: wrap; margin-bottom: 20px;">
                    <StatBox label="Containers Added" value={summary_containers_added} color="#22c55e" bg="rgba(34, 197, 94, 0.12)" />
                    <StatBox label="Containers Removed" value={summary_containers_removed} color="#ef4444" bg="rgba(239, 68, 68, 0.12)" />
                    <StatBox label="Containers Modified" value={summary_containers_modified} color="#eab308" bg="rgba(234, 179, 8, 0.12)" />
                    <StatBox label="Relationships Added" value={rel_added} color="#22c55e" bg="rgba(34, 197, 94, 0.12)" />
                    <StatBox label="Relationships Removed" value={rel_removed} color="#ef4444" bg="rgba(239, 68, 68, 0.12)" />
                </div>

                <p style="font-size: 14px; color: var(--color-text-muted);">{format!("Total changes: {}", summary_total_changes)}</p>
            </div>

            {/* Containers Added */}
            <Show when={move || has_added} fallback={|| view! { <></> }}>
                <div style="background: var(--color-surface-raised); border: 1px solid var(--color-border); border-radius: 12px; padding: 20px; box-shadow: var(--shadow-card);">
                    <h3 style="font-size: 14px; font-weight: 600; color: #22c55e; margin-bottom: 16px;">"Containers Added"</h3>
                    <div style="display: flex; flex-direction: column; gap: 8px;">
                        {containers_added.iter().map(|c| {
                            view! {
                                <div style="display: flex; align-items: center; gap: 12px; padding: 12px; background: rgba(34, 197, 94, 0.08); border: 1px solid rgba(34, 197, 94, 0.2); border-radius: 8px;">
                                    <span style="font-size: 12px; font-weight: 600; color: #22c55e; text-transform: uppercase; min-width: 60px;">"Added"</span>
                                    <div>
                                        <p style="font-size: 14px; font-weight: 500; color: var(--color-text-primary);">{c.name.clone()}</p>
                                        <p style="font-size: 12px; color: var(--color-text-muted);">{format!("{} — {}", c.technology, c.description)}</p>
                                    </div>
                                </div>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                </div>
            </Show>

            {/* Containers Removed */}
            <Show when={move || has_removed} fallback={|| view! { <></> }}>
                <div style="background: var(--color-surface-raised); border: 1px solid var(--color-border); border-radius: 12px; padding: 20px; box-shadow: var(--shadow-card);">
                    <h3 style="font-size: 14px; font-weight: 600; color: #ef4444; margin-bottom: 16px;">"Containers Removed"</h3>
                    <div style="display: flex; flex-direction: column; gap: 8px;">
                        {containers_removed.iter().map(|c| {
                            view! {
                                <div style="display: flex; align-items: center; gap: 12px; padding: 12px; background: rgba(239, 68, 68, 0.08); border: 1px solid rgba(239, 68, 68, 0.2); border-radius: 8px;">
                                    <span style="font-size: 12px; font-weight: 600; color: #ef4444; text-transform: uppercase; min-width: 60px;">"Removed"</span>
                                    <div>
                                        <p style="font-size: 14px; font-weight: 500; color: var(--color-text-primary);">{c.name.clone()}</p>
                                        <p style="font-size: 12px; color: var(--color-text-muted);">{format!("{} — {}", c.technology, c.description)}</p>
                                    </div>
                                </div>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                </div>
            </Show>

            {/* Containers Modified */}
            <Show when={move || has_modified} fallback={|| view! { <></> }}>
                <div style="background: var(--color-surface-raised); border: 1px solid var(--color-border); border-radius: 12px; padding: 20px; box-shadow: var(--shadow-card);">
                    <h3 style="font-size: 14px; font-weight: 600; color: #eab308; margin-bottom: 16px;">"Containers Modified"</h3>
                    <div style="display: flex; flex-direction: column; gap: 8px;">
                        {containers_modified.iter().map(|c| {
                            view! {
                                <div style="display: flex; align-items: center; gap: 12px; padding: 12px; background: rgba(234, 179, 8, 0.08); border: 1px solid rgba(234, 179, 8, 0.2); border-radius: 8px;">
                                    <span style="font-size: 12px; font-weight: 600; color: #eab308; text-transform: uppercase; min-width: 60px;">"Modified"</span>
                                    <div>
                                        <p style="font-size: 14px; font-weight: 500; color: var(--color-text-primary);">{c.name.clone()}</p>
                                        <p style="font-size: 12px; color: var(--color-text-muted);">
                                            {format!("{} → {} | {} → {}",
                                                c.before_technology.as_deref().unwrap_or("—"),
                                                c.after_technology.as_deref().unwrap_or("—"),
                                                c.before_description.as_deref().unwrap_or("—"),
                                                c.after_description.as_deref().unwrap_or("—")
                                            )}
                                        </p>
                                    </div>
                                </div>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                </div>
            </Show>

            {/* Diff Output (Mermaid) */}
            <DiagramViewer mermaid_code={diff_output} />
        </div>
    }
}

/// A single stat box with label, value, and color.
#[component]
fn StatBox(label: &'static str, value: usize, color: &'static str, bg: &'static str) -> impl IntoView {
    view! {
        <div style={format!("display: flex; flex-direction: column; align-items: center; gap: 4px; padding: 12px 20px; background: {}; border-radius: 8px; min-width: 120px;", bg)}>
            <span style={format!("font-size: 22px; font-weight: 700; color: {};", color)}>{value}</span>
            <span style="font-size: 12px; color: var(--color-text-secondary); text-align: center;">{label}</span>
        </div>
    }
}
