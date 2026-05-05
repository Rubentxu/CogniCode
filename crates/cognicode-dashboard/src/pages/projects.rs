//! Projects Page — SonarQube-style list with register + file browser

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;
use crate::state::ReactiveAppState;
use crate::api_client::ProjectInfoDto;
use crate::components::{Shell, LoadingSpinner, FileBrowser};

#[component]
pub fn ProjectsPage() -> impl IntoView {
    let state = expect_context::<ReactiveAppState>();
    let (projects, set_projects) = signal(Vec::<ProjectInfoDto>::new());
    let (show_register, set_show_register) = signal(false);

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
                {
                    let st = state.clone();
                    move || st.loading.get().then(|| view! { <LoadingSpinner message="Loading..." /> })
                }
                {
                    let st = state.clone();
                    move || show_register.get().then(|| view! {
                        <RegisterForm
                            state=st.clone()
                            set_projects=set_projects
                            set_show_register=set_show_register
                        />
                    })
                }
                {
                    let st = state.clone();
                    view! { <ProjectList projects=projects state=st /> }
                }
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
    let (show_browser, set_show_browser) = signal(false);

    let do_register = {
        let st = state;
        let _ = register_name;
        let _ = register_path;
        move |_| {
            let s = st.clone();
            let name = register_name.get();
            let path = register_path.get();
            spawn_local(async move {
                s.loading.set(true);
                match s.api.register_project(&name, &path).await {
                    Ok(info) => {
                        s.selected_project_name.set(Some(info.name.clone()));
                        s.project_path.set(info.path.clone());
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
                "Select a project directory. The dashboard will read from its " <code class="text-mono">".cognicode/cognicode.db"</code> " file."
            </p>
            <div class="space-y-4">
                <div>
                    <label class="text-caption text-text-muted uppercase mb-1 block">Project Name</label>
                    <input type="text" class="input" placeholder="Auto-detected from path"
                        prop:value={move || register_name.get()}
                        on:input=move |e| set_register_name.set(event_target_value(&e))
                    />
                </div>
                <div>
                    <label class="text-caption text-text-muted uppercase mb-1 block">Project Path</label>
                    <div class="flex gap-2">
                        <input type="text" class="input flex-1" placeholder="/path/to/project"
                            prop:value={move || register_path.get()}
                            on:input=move |e| {
                                let val = event_target_value(&e);
                                // Auto-detect name from path
                                if let Some(n) = val.rsplit('/').next() {
                                    if !n.is_empty() {
                                        set_register_name.set(n.to_string());
                                    }
                                }
                                set_register_path.set(val);
                            }
                        />
                        <button class="btn btn-secondary btn-sm"
                            on:click=move |_| set_show_browser.set(!show_browser.get())>
                            {move || if show_browser.get() { "Hide" } else { "Browse" }}
                        </button>
                    </div>
                </div>

                {move || show_browser.get().then(|| view! {
                    <FileBrowser
                        current_path=register_path
                        on_select=set_register_path
                        on_close=set_show_browser
                    />
                })}

                <button class="btn btn-primary" on:click=do_register>Register</button>
            </div>
        </div>
    }
}

#[component]
fn ProjectList(projects: ReadSignal<Vec<ProjectInfoDto>>, state: ReactiveAppState) -> impl IntoView {
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
                let name = p.name.clone();
                let path = p.path.clone();
                let st = state.clone();
                view! {
                    <div class="card hover:shadow-elevated transition-shadow cursor-pointer"
                        on:click=move |_| {
                            // Select this project for the dashboard
                            st.selected_project_name.set(Some(name.clone()));
                            st.project_path.set(path.clone());
                        }>
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
