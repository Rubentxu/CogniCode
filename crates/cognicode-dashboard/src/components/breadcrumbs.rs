//! Breadcrumbs component for navigation path display
//!
//! Shows the current location path: Home > Diagrams > [current diagram]

use leptos::prelude::*;
use wasm_bindgen::UnwrapThrowExt;

/// A single breadcrumb item
#[derive(Clone, Debug)]
pub struct BreadcrumbItem {
    pub label: String,
    pub href: Option<String>,
}

impl BreadcrumbItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self { label: label.into(), href: None }
    }

    pub fn with_href(label: impl Into<String>, href: impl Into<String>) -> Self {
        Self { label: label.into(), href: Some(href.into()) }
    }
}

/// Derive breadcrumbs from the current location path
fn derive_breadcrumbs(path: &str) -> Vec<BreadcrumbItem> {
    let mut items = vec![BreadcrumbItem::with_href("Home", "/")];

    if path.starts_with("/diagrams") {
        items.push(BreadcrumbItem::with_href("Diagrams", "/diagrams"));

        if path == "/diagrams" {
            // Just /diagrams - no additional breadcrumb
        } else if path == "/diagrams/diff" {
            items.push(BreadcrumbItem::new("Comparison"));
        } else if path.starts_with("/diagrams/") {
            items.push(BreadcrumbItem::new("Diagram"));
        }
    } else if path.starts_with("/issues") {
        items.push(BreadcrumbItem::with_href("Issues", "/issues"));
        if path.starts_with("/issues/") && path != "/issues" {
            items.push(BreadcrumbItem::new("Issue Detail"));
        }
    } else if path.starts_with("/projects") {
        items.push(BreadcrumbItem::with_href("Projects", "/projects"));
    } else if path == "/" {
        items.push(BreadcrumbItem::new("Dashboard"));
    } else {
        let label = path
            .trim_start_matches('/')
            .split('/')
            .next()
            .map(|s| {
                s.replace('-', " ")
                    .split_whitespace()
                    .map(|word| {
                        let mut chars = word.chars();
                        match chars.next() {
                            None => String::new(),
                            Some(c) => c.to_uppercase().chain(chars).collect(),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();

        if !label.is_empty() {
            items.push(BreadcrumbItem::new(label));
        }
    }

    items
}

/// Breadcrumbs navigation component
#[component]
pub fn Breadcrumbs() -> impl IntoView {
    let pathname = window().location().pathname().unwrap_throw();

    view! {
        <nav
            aria-label="Breadcrumb"
            style="
                display: flex;
                align-items: center;
                gap: 8px;
                margin-bottom: 16px;
                font-size: 14px;
            "
        >
            {move || {
                let crumbs = derive_breadcrumbs(&pathname);
                crumbs
                    .iter()
                    .enumerate()
                    .map(|(i, item)| {
                        let is_last = i == crumbs.len() - 1;
                        let label = item.label.clone();

                        // Separator
                        let separator = if i > 0 {
                            Some(view! {
                                <svg
                                    style="width: 16px; height: 16px; color: var(--color-text-muted); flex-shrink: 0;"
                                    viewBox="0 0 24 24"
                                    fill="none"
                                    stroke="currentColor"
                                    stroke-width="2"
                                >
                                    <path
                                        stroke-linecap="round"
                                        stroke-linejoin="round"
                                        d="M8.25 4.5l7.5 7.5-7.5 7.5"
                                    />
                                </svg>
                            })
                        } else {
                            None
                        };

                        // Content based on whether it's the last item and has href
                        let text_style = if is_last {
                            "color: var(--color-text-primary); font-weight: 600;".to_string()
                        } else {
                            "color: var(--color-text-secondary);".to_string()
                        };

                        let item_content = view! {
                            <span style={text_style}>{label}</span>
                        };

                        view! {
                            <span style="display: contents">
                                {separator}
                                {item_content}
                            </span>
                        }
                    })
                    .collect::<Vec<_>>()
            }}
        </nav>
    }
}
