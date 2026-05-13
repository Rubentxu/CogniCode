//! Export utilities for dashboard data
//!
//! Provides JSON/CSV export and URL sharing for dashboard data.

use leptos::prelude::*;
use wasm_bindgen::JsCast;

/// Export data as JSON file
pub fn export_json<T: serde::Serialize>(data: &T, filename: &str) {
    if let Ok(json) = serde_json::to_string_pretty(data) {
        download_text_file(&json, filename, "application/json");
    }
}

/// Export issues as CSV
pub fn export_issues_csv(issues: &[crate::state::IssueResult], filename: &str) {
    let mut csv = String::from("rule_id,message,severity,category,file,line,column,remediation_hint,effort_minutes\n");

    for issue in issues {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}\n",
            escape_csv(&issue.rule_id),
            escape_csv(&issue.message),
            escape_csv(&format!("{:?}", issue.severity)),
            escape_csv(&format!("{:?}", issue.category)),
            escape_csv(&issue.file),
            issue.line,
            issue.column.map_or(String::new(), |c| c.to_string()),
            escape_csv(&issue.remediation_hint.clone().unwrap_or_default()),
            issue.effort_minutes.map_or(String::new(), |e| e.to_string()),
        ));
    }

    download_text_file(&csv, filename, "text/csv");
}

/// Download a text file
fn download_text_file(content: &str, filename: &str, mime_type: &str) {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    let document = match window.document() {
        Some(d) => d,
        None => return,
    };

    // Create blob from text using Array
    let array = js_sys::Array::new();
    array.push(&content.into());

    let blob = match web_sys::Blob::new_with_u8_array_sequence(&array) {
        Ok(b) => b,
        Err(_) => return,
    };

    let url = match web_sys::Url::create_object_url_with_blob(&blob) {
        Ok(u) => u,
        Err(_) => return,
    };

    if let Ok(anchor) = document.create_element("a") {
        let _ = anchor.set_attribute("href", &url);
        let _ = anchor.set_attribute("download", filename);
        let _ = anchor.set_attribute("type", mime_type);
        if let Ok(html_anchor) = anchor.dyn_into::<web_sys::HtmlElement>() {
            let event = web_sys::MouseEvent::new("click").expect("Could not create click event");
            let _ = html_anchor.dispatch_event(&event);
        }
    }

    let _ = web_sys::Url::revoke_object_url(&url);
}

fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Copy text to clipboard using the Clipboard API
pub fn copy_to_clipboard(text: &str) -> Result<(), String> {
    let window = web_sys::window().ok_or("No window")?;
    let document = window.document().ok_or("No document")?;

    // Use clipboard API directly
    window.navigator().clipboard().write_text(text);

    Ok(())
}

/// Generate a shareable URL for the current dashboard state
#[derive(Clone)]
pub struct ShareableUrl {
    base_url: String,
    project_path: Option<String>,
    page: Option<String>,
    filters: Vec<(String, String)>,
}

impl ShareableUrl {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
            project_path: None,
            page: None,
            filters: Vec::new(),
        }
    }

    pub fn with_project(mut self, path: &str) -> Self {
        self.project_path = Some(path.to_string());
        self
    }

    pub fn with_page(mut self, page: &str) -> Self {
        self.page = Some(page.to_string());
        self
    }

    pub fn with_filter(mut self, key: &str, value: &str) -> Self {
        self.filters.push((key.to_string(), value.to_string()));
        self
    }

    pub fn build(&self) -> String {
        let mut url = self.base_url.clone();

        let mut params: Vec<String> = Vec::new();

        if let Some(ref path) = self.project_path {
            params.push(format!("project={}", urlencoding::encode(path)));
        }
        if let Some(ref page) = self.page {
            params.push(format!("page={}", urlencoding::encode(page)));
        }
        for (key, value) in &self.filters {
            params.push(format!("{}={}", urlencoding::encode(key), urlencoding::encode(value)));
        }

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        url
    }
}

/// Share button component - copies URL to clipboard
#[component]
pub fn ShareButton(
    url: String,
) -> impl IntoView {
    let (copied, set_copied) = signal(false);

    let handle_click = move |_| {
        if let Err(e) = copy_to_clipboard(&url) {
            log::warn!("Failed to copy to clipboard: {}", e);
        }
        set_copied.set(true);
        set_timeout(move || set_copied.set(false), std::time::Duration::from_secs(2));
    };

    view! {
        <button
            class="share-button"
            on:click={handle_click}
            title="Copy shareable URL to clipboard"
        >
            {move || if copied.get() { "Copied!" } else { "Share" }}
        </button>
    }
}

/// Export menu button with dropdown
#[component]
pub fn ExportMenuButton(
    children: Children,
) -> impl IntoView {
    let (open, set_open) = signal(false);

    let toggle = move |_| set_open.update(|v| *v = !*v);

    view! {
        <div class="export-menu-container">
            <button
                class="export-trigger"
                on:click={toggle}
            >
                {children()}
                <span class="caret">v</span>
            </button>
            <Show when={move || open.get()}>
                <div class="export-dropdown">
                    <slot />
                </div>
            </Show>
        </div>
    }
}
