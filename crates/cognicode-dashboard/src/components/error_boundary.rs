//! Error Boundary component for catching runtime errors
//!
//! Wraps children in an error boundary that catches any panics or errors
//! and displays a fallback UI instead of crashing the entire app.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

/// Error info to display when an error occurs
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ErrorInfo {
    pub message: String,
    pub details: Option<String>,
}

impl ErrorInfo {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(message: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            details: Some(details.into()),
        }
    }
}

/// Fallback UI shown when an error occurs
#[component]
pub fn ErrorFallback(error: ErrorInfo) -> impl IntoView {
    // Pre-extract values to avoid moving error
    let message = error.message;
    let details_text = error.details.unwrap_or_default();

    view! {
        <div style="display: flex; flex-direction: column; align-items: center; justify-content: center; padding: 48px 24px; text-align: center; min-height: 300px;">
            <div style="width: 64px; height: 64px; border-radius: 50%; background: var(--color-error-bg, #fef2f2); display: flex; align-items: center; justify-content: center; margin-bottom: 24px;">
                <svg style="width: 32px; height: 32px; color: var(--color-error, #dc2626);" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"/>
                </svg>
            </div>

            <h2 style="font-size: 20px; font-weight: 600; color: var(--color-text-primary, #1f2937); margin-bottom: 8px;">
                {"Something went wrong"}
            </h2>

            <p style="font-size: 14px; color: var(--color-text-secondary, #6b7280); margin-bottom: 16px; max-width: 400px;">
                {message}
            </p>

            <details style="text-align: left; width: 100%; max-width: 500px; margin-bottom: 24px;">
                <summary style="cursor: pointer; font-size: 13px; color: var(--color-text-muted, #9ca3af); margin-bottom: 8px;">
                    {"Technical Details"}
                </summary>
                <pre style="font-size: 12px; color: var(--color-text-secondary, #6b7280); background: var(--color-surface-hover, #f3f4f6); padding: 16px; border-radius: 8px; overflow-x: auto; white-space: pre-wrap; word-break: break-word;">
                    {details_text}
                </pre>
            </details>

            <div style="display: flex; gap: 12px;">
                <button
                    class="btn btn-primary"
                    style="padding: 10px 20px; font-size: 14px;"
                >
                    {"Reload Page"}
                </button>
                <a href="/" style="padding: 10px 20px; font-size: 14px; color: var(--color-text-secondary, #6b7280); text-decoration: none; display: inline-flex; align-items: center; border: 1px solid var(--color-border, #e5e7eb); border-radius: 8px; transition: all 0.15s ease;">
                    {"Go to Dashboard"}
                </a>
            </div>
        </div>
    }
}

/// Error Boundary component that catches errors in its children
///
/// Uses Suspense for loading states and provides error fallback.
#[component]
pub fn ErrorBoundary(children: ChildrenFn) -> impl IntoView {
    view! {
        <Suspense fallback={move || {
            view! {
                <div style="display: flex; align-items: center; justify-content: center; min-height: 200px;">
                    <LoadingSpinner message={Some("Loading...".to_string())} />
                </div>
            }
        }}>
            {children()}
        </Suspense>
    }
}

// Re-export LoadingSpinner for use in ErrorBoundary
use crate::components::LoadingSpinner;
