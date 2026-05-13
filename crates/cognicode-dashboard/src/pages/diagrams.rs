//! Diagrams Page — Generate and view architecture diagrams
//!
//! Supports C4, Sequence, StateMachine, Activity, and MultiLang diagrams.

use leptos::prelude::*;
use leptos_router::hooks::use_query;
use leptos_router::params::Params;
use wasm_bindgen_futures::spawn_local;
use crate::state::ReactiveAppState;
use crate::components::{Shell, LoadingSpinner};
use crate::api::diagrams::*;
use crate::components::diagram_viewer::{DiagramViewer, get_shared_diagram_state_from_url};

/// Query params for pre-filling project path from URL
#[derive(Params, Clone, Debug, PartialEq)]
pub struct ProjectPathQuery {
    pub project_path: Option<String>,
}

/// Diagrams page component
#[component]
pub fn DiagramsPage() -> impl IntoView {
    let _state = expect_context::<ReactiveAppState>();

    // Read project_path from query params if available
    let query_params = use_query::<ProjectPathQuery>();
    let initial_path = query_params.get()
        .ok()
        .and_then(|q| q.project_path.clone())
        .unwrap_or_else(|| String::from("/home/rubentxu/Proyectos/rust/CogniCode-diagram-f5"));

    // Form signals
    let (project_path, set_project_path) = signal(initial_path);
    let (diagram_type, set_diagram_type) = signal(DiagramType::C4);
    let (c4_level, set_c4_level) = signal(C4Level::Context);
    let (entry_symbol, set_entry_symbol) = signal(String::new());

    // Result signals
    let (generated_mermaid, set_generated_mermaid) = signal(Option::<String>::None);
    let (cached_diagrams, set_cached_diagrams) = signal(Vec::<CachedDiagramDto>::new());
    let (loading, set_loading) = signal(false);
    let (error_msg, set_error) = signal(Option::<String>::None);
    let (generating, set_generating) = signal(false);

    // Load cached diagrams on mount
    {
        spawn_local(async move {
            set_loading.set(true);
            match list_diagrams().await {
                Ok(resp) => set_cached_diagrams.set(resp.diagrams),
                Err(e) => set_error.set(Some(e)),
            }
            set_loading.set(false);
        });
    }

    // Check for shared diagram URL on mount
    {
        let set_generated_mermaid = set_generated_mermaid.clone();
        let set_diagram_type = set_diagram_type.clone();
        spawn_local(async move {
            if let Some(shared) = get_shared_diagram_state_from_url() {
                set_generated_mermaid.set(Some(shared.code));
                // Update diagram type selector based on shared state
                if !shared.diagram_type.is_empty() {
                    let dt = match shared.diagram_type.as_str() {
                        "c4" => DiagramType::C4,
                        "sequence" => DiagramType::Sequence,
                        "state_machine" => DiagramType::StateMachine,
                        "activity" => DiagramType::Activity,
                        "multi_lang" => DiagramType::MultiLang,
                        _ => DiagramType::C4,
                    };
                    set_diagram_type.set(dt);
                }
            }
        });
    }

    let generate_diagram = {
        move || {
            let path = project_path.get();
            let dtype = diagram_type.get();
            let level = c4_level.get();
            let entry = entry_symbol.get();

            spawn_local(async move {
                set_generating.set(true);
                set_error.set(None);
                set_generated_mermaid.set(None);

                let request = GenerateDiagramRequest {
                    project_path: path.clone(),
                    diagram_type: dtype.as_str().to_string(),
                    level: if dtype == DiagramType::C4 { Some(level.as_str().to_string()) } else { None },
                    entry_symbol: if dtype != DiagramType::C4 && dtype != DiagramType::MultiLang && !entry.is_empty() {
                        Some(entry.clone())
                    } else { None },
                    format: Some("mermaid".to_string()),
                };

                match generate_diagram(request).await {
                    Ok(resp) => {
                        set_generated_mermaid.set(Some(resp.mermaid_code));
                        // Refresh cached diagrams
                        match list_diagrams().await {
                            Ok(list_resp) => set_cached_diagrams.set(list_resp.diagrams),
                            Err(_) => {}
                        }
                    }
                    Err(e) => set_error.set(Some(e)),
                }
                set_generating.set(false);
            });
        }
    };

    let show_level_selector = move || diagram_type.get() == DiagramType::C4;
    let show_entry_symbol = move || {
        let dt = diagram_type.get();
        dt != DiagramType::C4 && dt != DiagramType::MultiLang
    };

    view! {
        <Shell>
            <div style="max-width: 1200px;">
                {/* Header */}
                <header style="margin-bottom: 32px;">
                    <h1 style="font-size: 28px; font-weight: 700; color: var(--color-text-primary); margin-bottom: 8px;">
                        "Diagrams"
                    </h1>
                    <p style="font-size: 15px; color: var(--color-text-secondary);">
                        "Generate architecture diagrams from your codebase using Mermaid"
                    </p>
                </header>

                <div style="display: grid; grid-template-columns: 1fr 320px; gap: 24px;">
                    {/* Left: Generator form + diagram */}
                    <div style="display: flex; flex-direction: column; gap: 24px;">
                        {/* Generator Card */}
                        <div style="
                            background: var(--color-surface-raised);
                            border: 1px solid var(--color-border);
                            border-radius: 12px;
                            padding: 24px;
                            box-shadow: var(--shadow-card);
                        ">
                            <h2 style="font-size: 16px; font-weight: 600; color: var(--color-text-primary); margin-bottom: 20px;">
                                "Generate Diagram"
                            </h2>

                            {/* Project Path */}
                            <div style="margin-bottom: 16px;">
                                <label style="display: block; font-size: 13px; font-weight: 500; color: var(--color-text-secondary); margin-bottom: 6px;">
                                    "Project Path"
                                </label>
                                <input
                                    type="text"
                                    value={project_path.get()}
                                    on:input=move |ev| set_project_path.set(event_target_value(&ev))
                                    placeholder="/path/to/project"
                                    style="
                                        width: 100%;
                                        padding: 10px 14px;
                                        font-size: 14px;
                                        font-family: monospace;
                                        color: var(--color-text-primary);
                                        background: var(--color-surface);
                                        border: 1px solid var(--color-border);
                                        border-radius: 8px;
                                        outline: none;
                                        box-sizing: border-box;
                                    "
                                />
                            </div>

                            {/* Diagram Type */}
                            <div style="margin-bottom: 16px;">
                                <label style="display: block; font-size: 13px; font-weight: 500; color: var(--color-text-secondary); margin-bottom: 6px;">
                                    "Diagram Type"
                                </label>
                                <select
                                    on:change=move |ev| {
                                        let val = event_target_value(&ev);
                                        let dt = match val.as_str() {
                                            "c4" => DiagramType::C4,
                                            "sequence" => DiagramType::Sequence,
                                            "state_machine" => DiagramType::StateMachine,
                                            "activity" => DiagramType::Activity,
                                            "multi_lang" => DiagramType::MultiLang,
                                            _ => DiagramType::C4,
                                        };
                                        set_diagram_type.set(dt);
                                    }
                                    style="
                                        width: 100%;
                                        padding: 10px 14px;
                                        font-size: 14px;
                                        color: var(--color-text-primary);
                                        background: var(--color-surface);
                                        border: 1px solid var(--color-border);
                                        border-radius: 8px;
                                        outline: none;
                                        cursor: pointer;
                                        box-sizing: border-box;
                                    "
                                >
                                    {DiagramType::all().iter().map(|dt| {
                                        let label = dt.label();
                                        let value = dt.as_str();
                                        let selected = *dt == diagram_type.get();
                                        view! {
                                            <option value={value} selected={selected}>{label}</option>
                                        }
                                    }).collect::<Vec<_>>()}
                                </select>
                            </div>

                            {/* C4 Level (conditional) */}
                            <Show when={show_level_selector}>
                                <div style="margin-bottom: 16px;">
                                    <label style="display: block; font-size: 13px; font-weight: 500; color: var(--color-text-secondary); margin-bottom: 6px;">
                                        "C4 Level"
                                    </label>
                                    <select
                                        on:change=move |ev| {
                                            let val = event_target_value(&ev);
                                            let level = match val.as_str() {
                                                "context" => C4Level::Context,
                                                "container" => C4Level::Container,
                                                "component" => C4Level::Component,
                                                "code" => C4Level::Code,
                                                _ => C4Level::Context,
                                            };
                                            set_c4_level.set(level);
                                        }
                                        style="
                                            width: 100%;
                                            padding: 10px 14px;
                                            font-size: 14px;
                                            color: var(--color-text-primary);
                                            background: var(--color-surface);
                                            border: 1px solid var(--color-border);
                                            border-radius: 8px;
                                            outline: none;
                                            cursor: pointer;
                                            box-sizing: border-box;
                                        "
                                    >
                                        {C4Level::all().iter().map(|lvl| {
                                            let label = lvl.label();
                                            let value = lvl.as_str();
                                            let selected = *lvl == c4_level.get();
                                            view! {
                                                <option value={value} selected={selected}>{label}</option>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </select>
                                </div>
                            </Show>

                            {/* Entry Symbol (conditional) */}
                            <Show when={show_entry_symbol}>
                                <div style="margin-bottom: 16px;">
                                    <label style="display: block; font-size: 13px; font-weight: 500; color: var(--color-text-secondary); margin-bottom: 6px;">
                                        "Entry Symbol"
                                    </label>
                                    <input
                                        type="text"
                                        value={entry_symbol.get()}
                                        on:input=move |ev| set_entry_symbol.set(event_target_value(&ev))
                                        placeholder="e.g., main, MyStruct, handle_request"
                                        style="
                                            width: 100%;
                                            padding: 10px 14px;
                                            font-size: 14px;
                                            font-family: monospace;
                                            color: var(--color-text-primary);
                                            background: var(--color-surface);
                                            border: 1px solid var(--color-border);
                                            border-radius: 8px;
                                            outline: none;
                                            box-sizing: border-box;
                                        "
                                    />
                                </div>
                            </Show>

                            {/* Generate Button */}
                            <button
                                on:click=move |_| generate_diagram()
                                disabled={generating.get()}
                                style="
                                    width: 100%;
                                    padding: 12px 20px;
                                    font-size: 14px;
                                    font-weight: 600;
                                    color: #ffffff;
                                    background: var(--color-accent-sky);
                                    border: none;
                                    border-radius: 8px;
                                    cursor: pointer;
                                    transition: all 0.15s ease;
                                    opacity: if generating.get() { 0.7 } else { 1.0 };
                                "
                            >
                                {if generating.get() {
                                    view! { "Generating..." }
                                } else {
                                    view! { "Generate Diagram" }
                                }}
                            </button>
                        </div>

                        {/* Diagram Display */}
                        <Show when={move || generated_mermaid.get().is_some()}>
                            <DiagramViewer
                                mermaid_code={generated_mermaid.get().unwrap()}
                                diagram_type={diagram_type.get().as_str().to_string()}
                            />
                        </Show>
                        <Show when={move || generating.get() && generated_mermaid.get().is_none()}>
                            <div style="
                                background: var(--color-surface-raised);
                                border: 1px solid var(--color-border);
                                border-radius: 12px;
                                padding: 48px;
                                text-align: center;
                            ">
                                <LoadingSpinner message="Generating diagram..." />
                            </div>
                        </Show>
                        <Show when={move || !generating.get() && generated_mermaid.get().is_none()}>
                            <div style="
                                background: var(--color-surface-raised);
                                border: 1px solid var(--color-border);
                                border-radius: 12px;
                                padding: 48px;
                                text-align: center;
                            ">
                                <svg style="width: 48px; height: 48px; color: var(--color-text-muted); margin: 0 auto 16px;" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M4 5a1 1 0 011-1h14a1 1 0 011 1v2a1 1 0 01-1 1H5a1 1 0 01-1-1V5zM4 13a1 1 0 011-1h6a1 1 0 011 1v6a1 1 0 01-1 1H5a1 1 0 01-1-1v-6zM16 13a1 1 0 011-1h2a1 1 0 011 1v6a1 1 0 01-1 1h-2a1 1 0 01-1-1v-6z"/>
                                </svg>
                                <p style="color: var(--color-text-muted); font-size: 14px;">
                                    "Configure options above and click Generate to view a diagram"
                                </p>
                            </div>
                        </Show>
                    </div>

                    {/* Right: Cached diagrams list */}
                    <div style="
                        background: var(--color-surface-raised);
                        border: 1px solid var(--color-border);
                        border-radius: 12px;
                        padding: 20px;
                        box-shadow: var(--shadow-card);
                        max-height: calc(100vh - 150px);
                        overflow-y: auto;
                    ">
                        <h2 style="font-size: 16px; font-weight: 600; color: var(--color-text-primary); margin-bottom: 16px;">
                            "Cached Diagrams"
                        </h2>

                        <Show when={move || loading.get()}>
                            <div style="padding: 24px; text-align: center;">
                                <p style="color: var(--color-text-muted); font-size: 13px;">"Loading..."</p>
                            </div>
                        </Show>
                        <Show when={move || error_msg.get().is_some()}>
                            <div style="
                                background: var(--color-accent-sunset);
                                border-radius: 8px;
                                padding: 12px;
                                margin-bottom: 16px;
                            ">
                                <p style="color: var(--color-text-primary); font-size: 13px;">{error_msg.get().unwrap()}</p>
                            </div>
                        </Show>
                        <Show when={move || !loading.get() && error_msg.get().is_none() && cached_diagrams.get().is_empty()}>
                            <div style="padding: 24px; text-align: center;">
                                <p style="color: var(--color-text-muted); font-size: 13px;">"No cached diagrams yet"</p>
                            </div>
                        </Show>
                        <Show when={move || !loading.get() && error_msg.get().is_none() && !cached_diagrams.get().is_empty()}>
                            <div style="display: flex; flex-direction: column; gap: 8px;">
                                {cached_diagrams.get().iter().map(|diagram| {
                                    let age_str = format_age(diagram.age_secs);
                                    view! {
                                        <div style="
                                            padding: 12px;
                                            background: var(--color-surface);
                                            border: 1px solid var(--color-border);
                                            border-radius: 8px;
                                            cursor: pointer;
                                            transition: all 0.15s ease;
                                        ">
                                            <div style="display: flex; justify-content: space-between; align-items: start; margin-bottom: 4px;">
                                                <span style="font-size: 13px; font-weight: 500; color: var(--color-text-primary);">
                                                    {diagram.diagram_type.clone()}
                                                </span>
                                                <span style="font-size: 11px; color: var(--color-text-muted);">
                                                    {age_str}
                                                </span>
                                            </div>
                                            <p style="font-size: 12px; color: var(--color-text-muted); font-family: monospace; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;">
                                                {diagram.project_path.clone()}
                                            </p>
                                            <div style="display: flex; gap: 8px; margin-top: 8px;">
                                                <span style="font-size: 11px; color: var(--color-text-muted);">
                                                    {format!("{} elements", diagram.element_count)}
                                                </span>
                                            </div>
                                        </div>
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                        </Show>
                    </div>
                </div>
            </div>
        </Shell>
    }
}

/// Format age in seconds to human-readable string
fn format_age(secs: u64) -> String {
    if secs < 60 {
        format!("{}s ago", secs)
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86400)
    }
}
