//! File Browser Component — Navigate filesystem to select project path

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FsEntry {
    pub name: String,
    pub is_dir: bool,
    pub has_cognicode: bool,
    pub path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct FsLsResponse {
    path: String,
    parent: Option<String>,
    entries: Vec<FsEntry>,
}

/// File browser that lets you navigate directories to find a project
#[component]
pub fn FileBrowser(
    current_path: ReadSignal<String>,
    on_select: WriteSignal<String>,
    on_close: WriteSignal<bool>,
) -> impl IntoView {
    let (entries, set_entries) = signal(Vec::<FsEntry>::new());
    let (browser_path, set_browser_path) = signal(String::new());
    let (loading, set_loading) = signal(false);

    // Load initial directory
    {
        let curr = current_path.get();
        let dir = if curr.is_empty() { "/".to_string() } else { curr };
        spawn_local(async move {
            set_loading.set(true);
            let url = format!("http://localhost:3000/api/fs/ls");
            if let Ok(req) = Request::post(&url)
                .json(&serde_json::json!({"path": dir}))
            {
                if let Ok(resp) = req.send().await {
                    if let Ok(data) = resp.json::<FsLsResponse>().await {
                        set_browser_path.set(data.path);
                        set_entries.set(data.entries);
                    }
                }
            }
            set_loading.set(false);
        });
    }

    let navigate_to = {
        let set_ent = set_entries;
        let set_bp = set_browser_path;
        let set_ld = set_loading;
        move |path: String| {
            spawn_local(async move {
                set_ld.set(true);
                let url = format!("http://localhost:3000/api/fs/ls");
                if let Ok(req) = Request::post(&url)
                    .json(&serde_json::json!({"path": path}))
                {
                    if let Ok(resp) = req.send().await {
                        if let Ok(data) = resp.json::<FsLsResponse>().await {
                            set_bp.set(data.path);
                            set_ent.set(data.entries);
                        }
                    }
                }
                set_ld.set(false);
            });
        }
    };

    view! {
        <div class="card mb-4">
            <div class="flex items-center justify-between mb-4">
                <div class="flex items-center gap-2 text-body-sm text-text-secondary">
                    <span class="text-caption text-text-muted uppercase">Browse: </span>
                    <span class="text-mono text-body-sm">{move || browser_path.get()}</span>
                </div>
                <button class="btn btn-secondary btn-sm"
                    on:click=move |_| on_close.set(true)>
                    Close
                </button>
            </div>

            {/* Quick access bar */}
            <div class="flex items-center gap-2 mb-3 pb-3 border-b border-border">
                <button class="btn btn-secondary btn-sm"
                    on:click={let n = navigate_to.clone(); move |_| n("/home".to_string())}>
                    "🏠 Home"
                </button>
                {move || {
                    let home = "/home/".to_string();
                    let user = browser_path.get();
                    if user.starts_with("/home/") {
                        let parts: Vec<&str> = user.split('/').collect();
                        if parts.len() >= 3 {
                            let user_home = format!("/home/{}", parts[2]);
                            let n = navigate_to.clone();
                            Some(view! {
                                <button class="btn btn-secondary btn-sm"
                                    on:click=move |_| n(user_home.clone())>
                                    "👤 /home/" {parts[2].to_string()}
                                </button>
                            }.into_any())
                        } else { None }
                    } else { None }
                }}
                <button class="btn btn-secondary btn-sm"
                    on:click={let n = navigate_to.clone(); move |_| n("/".to_string())}>
                    "📂 /"
                </button>
            </div>

            {move || {
                let path = browser_path.get();
                if path != "/" && !path.is_empty() {
                    let parent = path.rsplitn(2, '/').nth(1)
                        .map(|p| if p.is_empty() { "/" } else { p })
                        .unwrap_or("/")
                        .to_string();
                    let nav = navigate_to.clone();
                    Some(view! {
                        <div class="flex items-center gap-2 px-3 py-2 hover:bg-surface cursor-pointer rounded border-b border-border mb-2"
                            on:click=move |_| nav(parent.clone())>
                            <span class="text-brand font-medium">"📁 .."</span>
                        </div>
                    })
                } else { None }
            }}

            {/* Entries */}
            <div class="max-h-80 overflow-y-auto space-y-1">
                {move || {
                    let items = entries.get();
                    if loading.get() {
                        return view! {
                            <div class="text-body-sm text-text-muted p-4">"Loading..."</div>
                        }.into_any();
                    }
                    if items.is_empty() {
                        return view! {
                            <div class="text-body-sm text-text-muted p-4">"Empty directory"</div>
                        }.into_any();
                    }
                    let list: Vec<_> = items.iter().map(|entry| {
                        let icon = if entry.is_dir { "📁" } else { "📄" };
                        let suffix = if entry.has_cognicode { " 🟢" } else { "" };
                        let name = entry.name.clone();
                        let is_dir = entry.is_dir;
                        let entry_path = entry.path.clone();
                        let nav = navigate_to.clone();
                        // Clone signals for this entry (they're cheap to clone)
                        let on_sel = on_select;
                        let on_cl = on_close;

                        let click_path = entry_path.clone();
                        let nav2 = nav.clone();
                        // Clone again for the Select button
                        let sel_path = click_path.clone();
                        let sel = on_sel;
                        let cl = on_cl;

                        view! {
                            <div class="flex items-center gap-2 px-3 py-2 hover:bg-surface cursor-pointer rounded"
                                on:click=move |_| {
                                    if is_dir { nav2(click_path.clone()); }
                                }>
                                <span>{icon} {name}{suffix}</span>
                                {move || if is_dir {
                                    let p = sel_path.clone();
                                    let s = sel;
                                    let c = cl;
                                    Some(view! {
                                        <button class="btn btn-primary btn-sm ml-auto"
                                            on:click=move |_| {
                                                s.set(p.clone());
                                                c.set(true);
                                            }>
                                            Select
                                        </button>
                                    }.into_any())
                                } else { None }}
                            </div>
                        }
                    }).collect::<Vec<_>>();
                    view! { <div>{list}</div> }.into_any()
                }}
            </div>

            <div class="mt-4 pt-3 border-t border-border text-body-sm text-text-muted">
                "🟢 = Has .cognicode/cognicode.db · Click dir to enter · 'Select' to choose"
            </div>
        </div>
    }
}
