//! Code Explorer Page — Tree view of files and symbols

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;
use std::collections::HashMap;
use crate::state::ReactiveAppState;
use crate::components::{Shell, LoadingSpinner};

/// Symbol kinds for display
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    Trait,
    Impl,
    Module,
    File,
}

impl SymbolKind {
    pub fn label(&self) -> &'static str {
        match self {
            SymbolKind::Function => "fn",
            SymbolKind::Struct => "struct",
            SymbolKind::Enum => "enum",
            SymbolKind::Trait => "trait",
            SymbolKind::Impl => "impl",
            SymbolKind::Module => "mod",
            SymbolKind::File => "file",
        }
    }

    pub fn color_class(&self) -> &'static str {
        match self {
            SymbolKind::Function => "badge bg-blue-100 text-blue-800",
            SymbolKind::Struct => "badge bg-purple-100 text-purple-800",
            SymbolKind::Enum => "badge bg-yellow-100 text-yellow-800",
            SymbolKind::Trait => "badge bg-pink-100 text-pink-800",
            SymbolKind::Impl => "badge bg-green-100 text-green-800",
            SymbolKind::Module => "badge bg-gray-100 text-gray-800",
            SymbolKind::File => "badge bg-orange-100 text-orange-800",
        }
    }
}

/// A symbol in the code tree
#[derive(Clone, Debug)]
pub struct SymbolDetail {
    pub name: String,
    pub kind: SymbolKind,
    pub file: String,
    pub line: usize,
    pub message: Option<String>,
}

/// A node in the symbol tree (file -> symbols)
#[derive(Clone, Debug)]
pub struct SymbolTreeNode {
    pub name: String,
    pub kind: SymbolKind,
    pub path: String,
    pub line: usize,
    pub children: Vec<SymbolTreeNode>,
}

/// Code Explorer page component
#[component]
pub fn CodeExplorerPage() -> impl IntoView {
    let state = expect_context::<ReactiveAppState>();

    // Local signals for the tree
    let (symbol_tree, set_symbol_tree) = signal(Vec::<SymbolTreeNode>::new());
    let (selected_symbol, set_selected_symbol) = signal(Option::<SymbolDetail>::None);
    let (search_query, set_search_query) = signal(String::new());
    let (expanded_files, set_expanded_files) = signal(HashMap::<String, bool>::new());

    // Build symbol tree from issues (grouped by file)
    let build_tree_from_issues = {
        let state = state.clone();
        move || {
            let issues = state.issues.get();
            let mut file_map: HashMap<String, Vec<SymbolDetail>> = HashMap::new();

            for issue in &issues {
                let file_path = issue.file.clone();
                let symbol = SymbolDetail {
                    name: issue.rule_id.clone(),
                    kind: guess_symbol_kind_from_rule(&issue.rule_id),
                    file: issue.file.clone(),
                    line: issue.line,
                    message: Some(issue.message.clone()),
                };
                file_map.entry(file_path).or_default().push(symbol);
            }

            let tree: Vec<SymbolTreeNode> = file_map
                .into_iter()
                .map(|(path, symbols)| {
                    let file_name = std::path::Path::new(&path)
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.clone());

                    SymbolTreeNode {
                        name: file_name,
                        kind: SymbolKind::File,
                        path: path.clone(),
                        line: 0,
                        children: symbols
                            .into_iter()
                            .map(|s| SymbolTreeNode {
                                name: s.name.clone(),
                                kind: s.kind,
                                path: format!("{}:{}", s.file, s.line),
                                line: s.line,
                                children: Vec::new(),
                            })
                            .collect(),
                    }
                })
                .collect();

            set_symbol_tree.set(tree);
        }
    };

    // Load issues on mount
    {
        let st = state.clone();
        spawn_local(async move {
            st.load_issues(None, None, None, 1).await;
            build_tree_from_issues();
        });
    }

    // Rebuild tree when issues change
    {
        let state = state.clone();
        Effect::new(move |_| {
            let _ = state.issues.get();
            build_tree_from_issues();
        });
    }

    view! {
        <Shell>
            <div class="p-8">
                <header class="mb-8">
                    <h1 class="text-h1 text-text-primary">Code Explorer</h1>
                    <p class="text-body text-text-secondary mt-1">Browse files and symbols in your codebase</p>
                </header>

                {/* Search bar */}
                <div class="mb-6">
                    <div class="relative">
                        <svg class="absolute left-3 top-1/2 transform -translate-y-1/2 w-5 h-5 text-text-muted" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"/>
                        </svg>
                        <input
                            type="text"
                            placeholder="Filter symbols by name..."
                            class="w-full pl-10 pr-4 py-2 border border-border rounded-lg bg-surface-raised text-text-primary placeholder-text-muted focus:outline-none focus:ring-2 focus:ring-brand focus:border-transparent"
                            on:input=move |ev| {
                                set_search_query.set(event_target_value(&ev));
                            }
                        />
                    </div>
                </div>

                {/* Loading */}
                {
                    let st = state.clone();
                    move || {
                        if st.loading.get() {
                            Some(view! { <LoadingSpinner message="Loading code symbols..." /> })
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

                {/* Main content: tree + detail panel */}
                <div class="flex gap-6" style="min-height: 500px;">
                    {/* Left panel: Symbol tree */}
                    <div class="flex-1 overflow-auto" style="max-height: 600px;">
                        {
                            let tree = symbol_tree.get();
                            let query = search_query.get();
                            let expanded = expanded_files.get();
                            let is_loading = state.loading.get();

                            if tree.is_empty() && !is_loading {
                                view! {
                                    <div class="card p-8 text-center">
                                        <p class="text-body text-text-muted">"No symbols found. Run an analysis first."</p>
                                    </div>
                                }.into_any()
                            } else {
                                view! {
                                    <div class="space-y-1">
                                        {tree.iter().map(|node| {
                                            render_file_node(node, query.clone(), expanded.clone(), set_selected_symbol, set_expanded_files)
                                        }).collect::<Vec<_>>()}
                                    </div>
                                }.into_any()
                            }
                        }
                    </div>

                    {/* Right panel: Symbol detail */}
                    <div class="w-96 flex-shrink-0">
                        {
                            let symbol_opt = selected_symbol.get();
                            match symbol_opt {
                                Some(symbol) => {
                                    let kind_class = symbol.kind.color_class().to_string();
                                    let kind_label = symbol.kind.label().to_string();
                                    let name = symbol.name.clone();
                                    let file = symbol.file.clone();
                                    let line = symbol.line;
                                    view! {
                                        <div class="card p-6 sticky top-0">
                                            <h3 class="text-h3 text-text-primary mb-4">Symbol Details</h3>
                                            <div class="space-y-4">
                                                <div>
                                                    <span class={kind_class}>{kind_label}</span>
                                                </div>
                                                <div>
                                                    <p class="text-body-sm text-text-muted mb-1">Name</p>
                                                    <p class="text-body text-text-primary font-mono">{name}</p>
                                                </div>
                                                <div>
                                                    <p class="text-body-sm text-text-muted mb-1">Location</p>
                                                    <p class="text-body text-text-primary font-mono text-sm">
                                                        {file}:{line}
                                                    </p>
                                                </div>
                                                {symbol.message.as_ref().map(|msg| {
                                                    let msg_clone = msg.clone();
                                                    view! {
                                                        <div>
                                                            <p class="text-body-sm text-text-muted mb-1">Message</p>
                                                            <p class="text-body text-text-secondary">{msg_clone}</p>
                                                        </div>
                                                    }
                                                })}
                                            </div>
                                        </div>
                                    }.into_any()
                                },
                                None => {
                                    view! {
                                        <div class="card p-6 text-center">
                                            <p class="text-body text-text-muted">"Select a symbol to view details"</p>
                                        </div>
                                    }.into_any()
                                }
                            }
                        }
                    </div>
                </div>
            </div>
        </Shell>
    }
}

/// Guess symbol kind from rule ID (heuristic)
fn guess_symbol_kind_from_rule(rule_id: &str) -> SymbolKind {
    let rule_lower = rule_id.to_lowercase();
    if rule_lower.contains("function") || rule_lower.contains("method") {
        SymbolKind::Function
    } else if rule_lower.contains("struct") {
        SymbolKind::Struct
    } else if rule_lower.contains("enum") {
        SymbolKind::Enum
    } else if rule_lower.contains("trait") {
        SymbolKind::Trait
    } else if rule_lower.contains("impl") {
        SymbolKind::Impl
    } else if rule_lower.contains("module") || rule_lower.contains("mod") {
        SymbolKind::Module
    } else {
        SymbolKind::Function
    }
}

/// Render a file node with its symbols
fn render_file_node(
    node: &SymbolTreeNode,
    search_query: String,
    expanded_files: HashMap<String, bool>,
    set_selected_symbol: WriteSignal<Option<SymbolDetail>>,
    set_expanded_files: WriteSignal<HashMap<String, bool>>,
) -> impl IntoView {
    let is_expanded = expanded_files.get(&node.path).copied().unwrap_or(false);
    let node_path = node.path.clone();
    let node_name = node.name.clone();

    // Filter children based on search query
    let visible_children: Vec<&SymbolTreeNode> = if search_query.is_empty() {
        node.children.iter().collect()
    } else {
        node.children.iter()
            .filter(|c| c.name.to_lowercase().contains(&search_query.to_lowercase()))
            .collect()
    };

    let has_visible_children = !visible_children.is_empty();

    view! {
        <div>
            {/* File row */}
            <button
                class="w-full flex items-center gap-2 px-3 py-2 rounded-lg hover:bg-surface transition-colors text-left"
                on:click=move |_| {
                    set_expanded_files.update(|s| {
                        if s.get(&node_path).copied().unwrap_or(false) {
                            s.remove(&node_path);
                        } else {
                            s.insert(node_path.clone(), true);
                        }
                    });
                }
            >
                <svg
                    class="w-4 h-4 text-text-muted flex-shrink-0 transition-transform"
                    style:transform={if is_expanded { "rotate(90deg)" } else { "rotate(0deg)" }}
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                >
                    <path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7"/>
                </svg>

                <svg class="w-5 h-5 text-orange-500 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/>
                </svg>

                <span class="text-body text-text-primary flex-1 truncate">{node_name}</span>
                <span class={node.kind.color_class()}>{node.kind.label()}</span>
            </button>

            {/* Children */}
            {if is_expanded && has_visible_children {
                Some(view! {
                    <div style:padding-left="24px">
                        {visible_children.iter().map(|child| {
                            render_symbol_node(child, set_selected_symbol)
                        }).collect::<Vec<_>>()}
                    </div>
                })
            } else {
                None
            }}
        </div>
    }
}

/// Render a symbol node
fn render_symbol_node(
    node: &SymbolTreeNode,
    set_selected_symbol: WriteSignal<Option<SymbolDetail>>,
) -> impl IntoView {
    let node_name = node.name.clone();
    let node_kind = node.kind.clone();
    let node_path = node.path.clone();
    let node_line = node.line;

    view! {
        <button
            class="w-full flex items-center gap-2 px-3 py-2 rounded-lg hover:bg-surface transition-colors text-left"
            on:click=move |_| {
                set_selected_symbol.set(Some(SymbolDetail {
                    name: node_name.clone(),
                    kind: node_kind.clone(),
                    file: node_path.clone(),
                    line: node_line,
                    message: None,
                }));
            }
        >
            {build_symbol_icon(&node.kind)}

            <span class="text-body text-text-primary flex-1 truncate">{node.name.clone()}</span>
            <span class={node.kind.color_class()}>{node.kind.label()}</span>
            <span class="text-body-sm text-text-muted font-mono">{format!(":{}", node.line)}</span>
        </button>
    }
}

/// Build icon for symbol kind
fn build_symbol_icon(kind: &SymbolKind) -> impl IntoView {
    match kind {
        SymbolKind::Function => view! {
            <svg class="w-4 h-4 text-blue-500 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M12 4v4m0 0l-3-3m3 3l3-3M4 16v2a2 2 0 002 2h12a2 2 0 002-2v-2"/>
            </svg>
        },
        SymbolKind::Struct => view! {
            <svg class="w-4 h-4 text-purple-500 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10"/>
            </svg>
        },
        SymbolKind::Enum => view! {
            <svg class="w-4 h-4 text-yellow-500 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10"/>
            </svg>
        },
        SymbolKind::Trait => view! {
            <svg class="w-4 h-4 text-pink-500 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M4.318 6.318a4.5 4.5 0 000 6.364L12 20.364l7.682-7.682a4.5 4.5 0 00-6.364-6.364L12 7.636l-1.318-1.318a4.5 4.5 0 00-6.364 0z"/>
            </svg>
        },
        SymbolKind::Impl => view! {
            <svg class="w-4 h-4 text-green-500 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4"/>
            </svg>
        },
        SymbolKind::Module => view! {
            <svg class="w-4 h-4 text-gray-500 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"/>
            </svg>
        },
        SymbolKind::File => view! {
            <svg class="w-4 h-4 text-orange-500 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/>
            </svg>
        },
    }
}