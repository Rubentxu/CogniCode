//! Projects Page — SonarQube-style list with register

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;
use crate::state::ReactiveAppState;
use crate::api_client::ProjectInfoDto;
use crate::components::{Shell, LoadingSpinner};

#[component]
pub fn ProjectsPage() -> impl IntoView {
    let state = expect_context::<ReactiveAppState>();
    let (projects, set_projects) = signal(Vec::<ProjectInfoDto>::new());
    let (show_register, set_show_register) = signal(false);

    // Load on mount
    {
        let st = state.clone();
        spawn_local(async move {  
            if let Ok(list) = st.api.list_projects().await {
                set_projects.set(list.projects);
            }
        });
    }

    view! {
        <Shell>
            <div class="p-8">
                <header class="mb-8 flex items-center justify-between">
                    <div>
                        <h1 class="text-h1 text-text-primary">Projects</h1>
                        <p class="text-body text-text-secondary mt-1">Manage your CogniCode projects</p>
                    </div>
                    <button class="btn btn-primary"
                        on:click=move |_| set_show_register.set(!show_register.get())>
                        {move || if show_register.get() { "Cancel" } else { "+ Add Project" }}
                    </button>
                </header>
                <div>
                    {move || state.loading.get().then(|| view! {
                        <LoadingSpinner message="Loading..." />
                    })}
                </div>
                <div>
                    {move || show_register.get().then(|| view! {
                        <RegisterForm
                            state=state.clone()
                            set_projects=set_projects
                            set_show_register=set_show_register
                        />
                    })}
                </div>
                <ProjectList projects=projects />
            </div>
        </Shell>
    }
}

#[component]
fn RegisterForm(
    state: ReactiveAppState,
    set_projects: WriteSignal<Vec<ProjectInfoDto>>,
    set_show_register: WriteSignal<bool>,
) -> impl IntoView {
    let (register_name, set_register_name) = signal(String::new());
    let (register_path, set_register_path) = signal(String::new());

    let do_register = {
        let st = state;
        move |_| {
            let s = st.clone();
            let name = register_name.get();
            let path = register_path.get();
            spawn_local(async move {
                s.loading.set(true);
                match s.api.register_project(&name, &path).await {
                    Ok(info) => {
                        set_projects.update(|l| l.push(info));
                        set_show_register.set(false);
                    }
                    Err(e) => s.error.set(Some(e)),
                }
                s.loading.set(false);
            });
        }
    };

    view! {
        <div class="card mb-8">
            <h3 class="text-h3 text-text-primary mb-4">Register New Project</h3>
            <p class="text-body-sm text-text-secondary mb-4">
                "Enter the project path. The dashboard reads from " <code class="text-mono">".cognicode/cognicode.db"</code>
            </p>
            <div class="space-y-4">
                <div>
                    <label class="text-caption text-text-muted uppercase mb-1 block">Name</label>
                    <input type="text" class="input" placeholder="My Project"
                        on:input=move |e| set_register_name.set(event_target_value(&e)) />
                </div>
                <div>
                    <label class="text-caption text-text-muted uppercase mb-1 block">Path</label>
                    <input type="text" class="input" placeholder="/path/to/project"
                        on:input=move |e| set_register_path.set(event_target_value(&e)) />
                </div>
                <button class="btn btn-primary" on:click=do_register>Register</button>
            </div>
        </div>
    }
}

#[component]
fn ProjectList(projects: ReadSignal<Vec<ProjectInfoDto>>) -> impl IntoView {
    move || {
        let list = projects.get();
        if list.is_empty() {
            view! {
                <div class="card text-center py-12">
                    <p class="text-h3 text-text-muted">"No projects registered"</p>
                    <p class="text-body text-text-secondary mt-2">"Click '+ Add Project' to register your first project"</p>
                </div>
            }.into_any()
        } else {
            let items: Vec<_> = list.into_iter().map(|p| {
                let rc = match p.rating.as_str() { "A"|"B" => "bg-rating-a", "C" => "bg-rating-c", _ => "bg-rating-e" };
                let gc = if p.quality_gate_status == "PASSED" { "badge-success" } else { "badge-error" };
                let ts = p.last_analysis.as_ref().map(|t| t[..19].to_string()).unwrap_or_else(|| "Never".to_string());
                view! {
                    <div class="card">
                        <div class="flex items-start justify-between mb-4">
                            <div class="flex items-center gap-4">
                                <span class={format!("w-10 h-10 rounded-xl flex items-center justify-center text-white font-bold {}", rc)}>{p.rating}</span>
                                <div><h3 class="text-h3 text-text-primary">{p.name}</h3><p class="text-mono text-body-sm text-text-muted">{p.path}</p></div>
                            </div>
                            <div class="flex items-center gap-3">
                                <span class={format!("badge {}", gc)}>{p.quality_gate_status}</span>
                                <span class="text-body-sm text-text-muted">{ts}</span>
                            </div>
                        </div>
                        <div class="grid grid-cols-4 gap-4 pt-4 border-t border-border">
                            <div><p class="text-caption text-text-muted">Issues</p><p class="text-h2 text-text-primary">{p.total_issues}</p></div>
                            <div><p class="text-caption text-text-muted">Debt</p><p class="text-h2 text-text-primary">{p.debt_minutes}" min"</p></div>
                            <div><p class="text-caption text-text-muted">Files</p><p class="text-h2 text-text-primary">{p.files_changed}</p></div>
                            <div><p class="text-caption text-text-muted">Runs</p><p class="text-h2 text-text-primary">{p.history_count}</p></div>
                        </div>
                    </div>
                }
            }).collect::<Vec<_>>();
            view! { <div class="space-y-4">{items}</div> }.into_any()
        }
    }
}
