//! Loading spinner component

use leptos::prelude::*;

#[component]
pub fn LoadingSpinner(message: Option<String>) -> impl IntoView {
    view! {
        <div style="display: flex; flex-direction: column; align-items: center; justify-content: center; padding: 48px; gap: 16px;">
            <div style="width: 48px; height: 48px; border: 4px solid var(--color-border); border-top-color: var(--color-brand); border-radius: 50%; animation: spin 0.8s linear infinite;"></div>
            {message.map(|msg| view! { <p style="font-size: 14px; color: var(--color-text-muted); text-align: center;">{msg}</p> })}
        </div>
    }
}