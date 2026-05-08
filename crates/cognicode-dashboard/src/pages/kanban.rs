//! Kanban Board Page — Simple issue tracking with agent task creation

use leptos::prelude::*;
use std::sync::Arc;
use wasm_bindgen_futures::spawn_local;
use crate::state::ReactiveAppState;
use crate::api_client::CreateTaskRequest;
use crate::components::{Shell, LoadingSpinner};
use serde::{Deserialize, Serialize};

// ============================================================================
// DTOs
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanbanCard {
    pub id: String,
    pub rule_id: String,
    pub message: String,
    pub severity: String,
    pub file: String,
    pub line: usize,
    pub column: String,
}

impl From<crate::api_client::IssueDto> for KanbanCard {
    fn from(issue: crate::api_client::IssueDto) -> Self {
        Self {
            id: format!("{}-{}-{}", issue.rule_id, issue.file, issue.line),
            rule_id: issue.rule_id,
            message: issue.message,
            severity: issue.severity,
            file: issue.file,
            line: issue.line,
            column: "todo".to_string(),
        }
    }
}

// ============================================================================
// Component
// ============================================================================

#[component]
pub fn KanbanPage() -> impl IntoView {
    let state = expect_context::<ReactiveAppState>();

    // Signals
    let (loading, set_loading) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);
    let (issues, set_issues) = signal(Vec::<KanbanCard>::new());

    // Load data
    let load_data = {
        let state = state.clone();
        let set_issues = set_issues.clone();
        let set_loading = set_loading.clone();
        let set_error = set_error.clone();
        move || {
            let project_path = state.project_path.get();
            if project_path.is_empty() {
                set_error.set(Some("No project selected".to_string()));
                return;
            }
            set_loading.set(true);
            set_error.set(None);

            let api = state.api.clone();
            spawn_local(async move {
                match api.get_issues(&project_path, None, None, None, 1, 100).await {
                    Ok(resp) => {
                        let cards: Vec<KanbanCard> =
                            resp.issues.into_iter().map(KanbanCard::from).collect();
                        set_issues.set(cards);
                    }
                    Err(e) => set_error.set(Some(e)),
                }
                set_loading.set(false);
            });
        }
    };

    // Initial load
    {
        load_data();
    }

    // Precompute column card lists as derived signals
    let todo_cards = Signal::derive(move || {
        issues
            .get()
            .iter()
            .filter(|c| c.column == "todo")
            .cloned()
            .collect::<Vec<_>>()
    });

    let in_progress_cards = Signal::derive(move || {
        issues
            .get()
            .iter()
            .filter(|c| c.column == "in_progress")
            .cloned()
            .collect::<Vec<_>>()
    });

    let fixed_cards = Signal::derive(move || {
        issues
            .get()
            .iter()
            .filter(|c| c.column == "fixed")
            .cloned()
            .collect::<Vec<_>>()
    });

    let verified_cards = Signal::derive(move || {
        issues
            .get()
            .iter()
            .filter(|c| c.column == "verified")
            .cloned()
            .collect::<Vec<_>>()
    });

    // Fix issue action wrapped in Arc<Box<dyn Fn + Send + Sync>> for cheap cloning
    let fix_issue_action: Arc<Box<dyn Fn(KanbanCard) + Send + Sync>> = Arc::new(Box::new({
        let state = state.clone();
        let set_error = set_error.clone();
        let set_issues = set_issues.clone();
        let set_loading = set_loading.clone();
        move |card: KanbanCard| {
            let _project_path = state.project_path.get();
            let api = state.api.clone();
            let se = set_error.clone();
            let si = set_issues.clone();
            let sl = set_loading.clone();

            let payload = serde_json::json!({
                "rule_id": card.rule_id,
                "file": card.file,
                "line": card.line,
            });
            let request = CreateTaskRequest {
                task_type: "fix_issue".to_string(),
                priority: Some(5),
                payload_json: serde_json::to_string(&payload).unwrap_or_default(),
                created_by: Some("dashboard".to_string()),
            };

            spawn_local(async move {
                match api.create_task(&request).await {
                    Ok(_) => {
                        sl.set(true);
                        let fresh_api = api.clone();
                        let fresh_path = state.project_path.get();
                        match fresh_api
                            .get_issues(&fresh_path, None, None, None, 1, 100)
                            .await
                        {
                            Ok(resp) => {
                                let cards: Vec<KanbanCard> =
                                    resp.issues.into_iter().map(KanbanCard::from).collect();
                                si.set(cards);
                            }
                            Err(e) => se.set(Some(e)),
                        }
                        sl.set(false);
                    }
                    Err(e) => se.set(Some(e)),
                }
            });
        }
    }));

    view! {
        <Shell>
            <div class="p-8">
                <header class="mb-8 flex items-center justify-between">
                    <div>
                        <h1 class="text-h1 text-text-primary">Kanban Board</h1>
                        <p class="text-body text-text-secondary mt-1">
                            Track code quality issues and create fix tasks
                        </p>
                    </div>
                    <button class="btn btn-secondary" on:click=move |_| load_data()>
                        "Refresh"
                    </button>
                </header>

                {/* Error */}
                {
                    let err = error;
                    move || {
                        err.get().map(|msg| {
                            view! {
                                <div class="card bg-accent-sunset mb-6">
                                    <p class="text-body text-severity-critical">{msg}</p>
                                </div>
                            }
                        })
                    }
                }

                {/* Loading */}
                {
                    let loading = loading;
                    move || {
                        if loading.get() {
                            Some(view! { <LoadingSpinner message="Loading board..." /> })
                        } else {
                            None
                        }
                    }
                }

                {/* Board */}
                {
                    let fix_action = fix_issue_action.clone();
                    move || {
                        let is_empty = todo_cards.get().is_empty()
                            && in_progress_cards.get().is_empty()
                            && fixed_cards.get().is_empty()
                            && verified_cards.get().is_empty();

                        if is_empty {
                            view! {
                                <div class="card p-8 text-center">
                                    <p class="text-body text-text-muted">"No issues found"</p>
                                </div>
                            }
                            .into_any()
                        } else {
                            view! {
                                <div class="grid grid-cols-4 gap-4">
                                    {/* To Do column */}
                                    <div class="flex flex-col">
                                        <div class="flex items-center justify-between mb-3 px-1">
                                            <h3 class="text-h4 font-semibold text-blue-600">"To Do"</h3>
                                            <span class="badge bg-surface-raised text-text-muted">
                                                {todo_cards.get().len()}
                                            </span>
                                        </div>
                                        <div class="flex-1 bg-surface rounded-lg p-2 space-y-2">
                                            {todo_cards
                                                .get()
                                                .iter()
                                                .map(|card| {
                                                    let fix = fix_action.clone();
                                                    let c = card.clone();
                                                    view! {
                                                        <div class="card p-3">
                                                            <div class="flex items-start justify-between gap-2 mb-2">
                                                                <span
                                                                    class="text-body-sm font-mono text-brand
                                                                           truncate flex-1"
                                                                >
                                                                    {card.rule_id.clone()}
                                                                </span>
                                                                <span
                                                                    class={format!(
                                                                        "badge text-xs px-1.5 py-0.5 {}",
                                                                        match card.severity.to_lowercase().as_str()
                                                                        {
                                                                            "blocker" => {
                                                                                "bg-red-100 text-red-800"
                                                                            }
                                                                            "critical" => {
                                                                                "bg-orange-100 text-orange-800"
                                                                            }
                                                                            "major" => {
                                                                                "bg-yellow-100 text-yellow-800"
                                                                            }
                                                                            "minor" => {
                                                                                "bg-blue-100 text-blue-800"
                                                                            }
                                                                            _ => "bg-gray-100 text-gray-800",
                                                                        }
                                                                    )}
                                                                >
                                                                    {card.severity.to_uppercase()}
                                                                </span>
                                                            </div>
                                                            <p
                                                                class="text-body-sm text-text-secondary
                                                                       line-clamp-2 mb-2"
                                                            >
                                                                {card.message.clone()}
                                                            </p>
                                                            <p class="text-caption text-text-muted mb-3 font-mono">
                                                                {format!(
                                                                    "{}:{}",
                                                                    card
                                                                        .file
                                                                        .split('/')
                                                                        .last()
                                                                        .unwrap_or(&card.file),
                                                                    card.line
                                                                )}
                                                            </p>
                                                            <button
                                                                class="btn btn-primary btn-sm w-full"
                                                                on:click=move |_| (*fix)(c.clone())
                                                            >
                                                                "Fix this issue"
                                                            </button>
                                                        </div>
                                                    }
                                                })
                                                .collect::<Vec<_>>()}
                                        </div>
                                    </div>

                                    {/* In Progress column */}
                                    <div class="flex flex-col">
                                        <div class="flex items-center justify-between mb-3 px-1">
                                            <h3 class="text-h4 font-semibold text-yellow-600">
                                                "In Progress"
                                            </h3>
                                            <span class="badge bg-surface-raised text-text-muted">
                                                {in_progress_cards.get().len()}
                                            </span>
                                        </div>
                                        <div
                                            class="flex-1 bg-surface rounded-lg p-2 space-y-2
                                                   min-h-[200px]"
                                        >
                                            {in_progress_cards
                                                .get()
                                                .iter()
                                                .map(|card| {
                                                    view! {
                                                        <div class="card p-3 opacity-60">
                                                            <span class="text-body-sm font-mono text-brand">
                                                                {card.rule_id.clone()}
                                                            </span>
                                                            <p class="text-caption text-text-muted">
                                                                {card.message.clone()}
                                                            </p>
                                                        </div>
                                                    }
                                                })
                                                .collect::<Vec<_>>()}
                                        </div>
                                    </div>

                                    {/* Fixed column */}
                                    <div class="flex flex-col">
                                        <div class="flex items-center justify-between mb-3 px-1">
                                            <h3 class="text-h4 font-semibold text-green-600">"Fixed"</h3>
                                            <span class="badge bg-surface-raised text-text-muted">
                                                {fixed_cards.get().len()}
                                            </span>
                                        </div>
                                        <div
                                            class="flex-1 bg-surface rounded-lg p-2 space-y-2
                                                   min-h-[200px]"
                                        >
                                            {fixed_cards
                                                .get()
                                                .iter()
                                                .map(|card| {
                                                    view! {
                                                        <div class="card p-3 opacity-60">
                                                            <span class="text-body-sm font-mono text-brand">
                                                                {card.rule_id.clone()}
                                                            </span>
                                                            <p class="text-caption text-text-muted">
                                                                {card.message.clone()}
                                                            </p>
                                                        </div>
                                                    }
                                                })
                                                .collect::<Vec<_>>()}
                                        </div>
                                    </div>

                                    {/* Verified column */}
                                    <div class="flex flex-col">
                                        <div class="flex items-center justify-between mb-3 px-1">
                                            <h3 class="text-h4 font-semibold text-purple-600">
                                                "Verified"
                                            </h3>
                                            <span class="badge bg-surface-raised text-text-muted">
                                                {verified_cards.get().len()}
                                            </span>
                                        </div>
                                        <div
                                            class="flex-1 bg-surface rounded-lg p-2 space-y-2
                                                   min-h-[200px]"
                                        >
                                            {verified_cards
                                                .get()
                                                .iter()
                                                .map(|card| {
                                                    view! {
                                                        <div class="card p-3 opacity-60">
                                                            <span class="text-body-sm font-mono text-brand">
                                                                {card.rule_id.clone()}
                                                            </span>
                                                            <p class="text-caption text-text-muted">
                                                                {card.message.clone()}
                                                            </p>
                                                        </div>
                                                    }
                                                })
                                                .collect::<Vec<_>>()}
                                        </div>
                                    </div>
                                </div>
                            }
                            .into_any()
                        }
                    }
                }
            </div>
        </Shell>
    }
}
