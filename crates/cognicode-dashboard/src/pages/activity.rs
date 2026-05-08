//! Activity Timeline Page — Timeline of agent tool usage

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;
use crate::state::ReactiveAppState;
use crate::components::{Shell, LoadingSpinner};

/// Timeline event for activity page
#[derive(Clone, Debug)]
pub struct TimelineEvent {
    pub tool_name: String,
    pub timestamp: String,
    pub duration_ms: f64,
    pub status: String,
}

/// Activity summary stats
#[derive(Clone, Debug, Default)]
pub struct ActivitySummary {
    pub total_calls: usize,
    pub success_rate: f64,
    pub avg_duration_ms: f64,
    pub most_used_tool: Option<String>,
}

/// Activity Timeline page component
#[component]
pub fn ActivityPage() -> impl IntoView {
    let state = expect_context::<ReactiveAppState>();

    // Local signals
    let (timeline_events, set_timeline_events) = signal(Vec::<TimelineEvent>::new());
    let (summary, set_summary) = signal(ActivitySummary::default());
    let (filter_tool, set_filter_tool) = signal(String::new());
    let (filter_status, set_filter_status) = signal(String::new());

    // Load agent stats on mount
    {
        let st = state.clone();
        spawn_local(async move {
            st.load_agent_stats(None).await;
        });
    }

    // Transform agent stats into timeline events and compute summary
    {
        let state = state.clone();
        Effect::new(move |_| {
            let stats = state.agent_stats.get();
            if stats.is_empty() {
                return;
            }

            // Build timeline events from stats
            let mut events: Vec<TimelineEvent> = Vec::new();
            for stat in &stats {
                let status = if stat.result_status_breakdown.success > 0 {
                    "success".to_string()
                } else if stat.result_status_breakdown.error > 0 {
                    "error".to_string()
                } else {
                    "other".to_string()
                };

                events.push(TimelineEvent {
                    tool_name: stat.tool_name.clone(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    duration_ms: stat.avg_duration_ms,
                    status,
                });
            }

            set_timeline_events.set(events);

            // Compute summary
            let total_calls: usize = stats.iter().map(|s| s.count).sum();
            let total_success: usize = stats.iter().map(|s| s.result_status_breakdown.success).sum();
            let total_duration: f64 = stats.iter().map(|s| s.avg_duration_ms * s.count as f64).sum();
            let avg_duration = if total_calls > 0 { total_duration / total_calls as f64 } else { 0.0 };
            let success_rate = if total_calls > 0 { total_success as f64 / total_calls as f64 * 100.0 } else { 0.0 };
            let most_used = stats.iter().max_by_key(|s| s.count).map(|s| s.tool_name.clone());

            set_summary.set(ActivitySummary {
                total_calls,
                success_rate,
                avg_duration_ms: avg_duration,
                most_used_tool: most_used,
            });
        });
    }

    view! {
        <Shell>
            <div class="p-8">
                <header class="mb-8">
                    <h1 class="text-h1 text-text-primary">Activity Timeline</h1>
                    <p class="text-body text-text-secondary mt-1">Timeline of agent tool usage and results</p>
                </header>

                {/* Summary cards */}
                {
                    let s = summary.get();
                    view! {
                        <div class="grid grid-cols-4 gap-4 mb-8">
                            <div class="card p-4">
                                <p class="text-body-sm text-text-muted mb-1">Total Calls</p>
                                <p class="text-h2 text-text-primary">{s.total_calls}</p>
                            </div>
                            <div class="card p-4">
                                <p class="text-body-sm text-text-muted mb-1">Success Rate</p>
                                <p class="text-h2 text-accent-ocean">{format!("{:.1}%", s.success_rate)}</p>
                            </div>
                            <div class="card p-4">
                                <p class="text-body-sm text-text-muted mb-1">Avg Duration</p>
                                <p class="text-h2 text-text-primary">{format!("{:.1}ms", s.avg_duration_ms)}</p>
                            </div>
                            <div class="card p-4">
                                <p class="text-body-sm text-text-muted mb-1">Most Used Tool</p>
                                <p class="text-h3 text-text-primary font-mono truncate">
                                    {s.most_used_tool.unwrap_or_else(|| "—".to_string())}
                                </p>
                            </div>
                        </div>
                    }
                }

                {/* Filter bar */}
                <div class="flex gap-4 mb-6">
                    <div class="flex-1">
                        <div class="relative">
                            <svg class="absolute left-3 top-1/2 transform -translate-y-1/2 w-5 h-5 text-text-muted" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"/>
                            </svg>
                            <input
                                type="text"
                                placeholder="Filter by tool name..."
                                class="w-full pl-10 pr-4 py-2 border border-border rounded-lg bg-surface-raised text-text-primary placeholder-text-muted focus:outline-none focus:ring-2 focus:ring-brand focus:border-transparent"
                                on:input=move |ev| {
                                    set_filter_tool.set(event_target_value(&ev));
                                }
                            />
                        </div>
                    </div>
                    <div class="w-48">
                        <select
                            class="w-full px-4 py-2 border border-border rounded-lg bg-surface-raised text-text-primary focus:outline-none focus:ring-2 focus:ring-brand focus:border-transparent"
                            on:change=move |ev| {
                                set_filter_status.set(event_target_value(&ev));
                            }
                        >
                            <option value="">All Status</option>
                            <option value="success">Success</option>
                            <option value="error">Error</option>
                            <option value="other">Other</option>
                        </select>
                    </div>
                </div>

                {/* Loading */}
                {
                    let st = state.clone();
                    move || {
                        if st.loading.get() {
                            Some(view! { <LoadingSpinner message="Loading activity timeline..." /> })
                        } else {
                            None
                        }
                    }
                }

                {/* Error */}
                {
                    let st = state.clone();
                    move || {
                        st.error.get().map(|msg| {
                            view! {
                                <div class="card bg-accent-sunset mb-6">
                                    <p class="text-body text-severity-critical">{msg}</p>
                                </div>
                            }
                        })
                    }
                }

                {/* Empty state */}
                {
                    let st = state.clone();
                    move || {
                        let events = timeline_events.get();
                        let loading = st.loading.get();
                        let error = st.error.get();
                        if !loading && error.is_none() && events.is_empty() {
                            Some(view! {
                                <div class="card p-8 text-center">
                                    <p class="text-body text-text-muted">"No activity recorded yet"</p>
                                </div>
                            })
                        } else {
                            None
                        }
                    }
                }

                {/* Timeline */}
                {
                    let events = timeline_events.get();
                    let tool_filter = filter_tool.get().to_lowercase();
                    let status_filter = filter_status.get().to_lowercase();
                    let is_loading = state.loading.get();

                    let filtered: Vec<TimelineEvent> = events
                        .into_iter()
                        .filter(|e| {
                            let tool_match = tool_filter.is_empty() || e.tool_name.to_lowercase().contains(&tool_filter);
                            let status_match = status_filter.is_empty() || e.status.to_lowercase() == status_filter;
                            tool_match && status_match
                        })
                        .collect();

                    if filtered.is_empty() && !is_loading {
                        view! {
                            <div class="card p-8 text-center">
                                <p class="text-body text-text-muted">"No events match your filters"</p>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <div class="relative">
                                {/* Vertical timeline line */}
                                <div class="absolute left-6 top-0 bottom-0 w-0.5 bg-border" />

                                <div class="space-y-4">
                                    {filtered.iter().map(|event| {
                                        let status_color = match event.status.to_lowercase().as_str() {
                                            "success" => "bg-green-500",
                                            "error" => "bg-red-500",
                                            _ => "bg-yellow-500",
                                        };

                                        let status_badge = match event.status.to_lowercase().as_str() {
                                            "success" => "badge bg-green-100 text-green-800",
                                            "error" => "badge bg-red-100 text-red-800",
                                            _ => "badge bg-yellow-100 text-yellow-800",
                                        };

                                        let tool_name = event.tool_name.clone();
                                        let status = event.status.clone();
                                        let timestamp = event.timestamp.clone();
                                        let duration = event.duration_ms;

                                        view! {
                                            <div class="relative flex items-start gap-4 pl-12">
                                                {/* Timeline dot */}
                                                <div class={format!("absolute left-4 w-4 h-4 rounded-full {} border-2 border-surface-raised", status_color)} />

                                                {/* Event card */}
                                                <div class="card flex-1 p-4">
                                                    <div class="flex items-start justify-between">
                                                        <div>
                                                            <div class="flex items-center gap-2 mb-1">
                                                                <span class="text-body font-medium text-text-primary font-mono">
                                                                    {tool_name}
                                                                </span>
                                                                <span class={status_badge}>{status.to_uppercase()}</span>
                                                            </div>
                                                            <p class="text-body-sm text-text-muted">
                                                                {timestamp}
                                                            </p>
                                                        </div>
                                                        <div class="text-right">
                                                            <p class="text-body font-medium text-text-primary">
                                                                {format!("{:.1}ms", duration)}
                                                            </p>
                                                            <p class="text-body-sm text-text-muted">duration</p>
                                                        </div>
                                                    </div>
                                                </div>
                                            </div>
                                        }
                                    }).collect::<Vec<_>>()}
                                </div>
                            </div>
                        }.into_any()
                    }
                }
            </div>
        </Shell>
    }
}