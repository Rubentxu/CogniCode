//! Live Updates UI components
//!
//! Toggle button and status display for live diagram updates.

use leptos::prelude::*;

/// Live updates state
#[derive(Clone)]
pub struct LiveState {
    /// Whether live updates are enabled
    pub enabled: RwSignal<bool>,
    /// Whether connected to server
    pub connected: RwSignal<bool>,
    /// Last change message
    pub last_change: RwSignal<Option<String>>,
}

impl LiveState {
    pub fn new() -> Self {
        Self {
            enabled: RwSignal::new(true),
            connected: RwSignal::new(false),
            last_change: RwSignal::new(None),
        }
    }

    pub fn toggle(&self) {
        self.enabled.update(|v| *v = !*v);
    }

    pub fn set_connected(&self, connected: bool) {
        self.connected.set(connected);
    }

    pub fn set_last_change(&self, msg: String) {
        self.last_change.set(Some(msg));
    }
}

impl Default for LiveState {
    fn default() -> Self {
        Self::new()
    }
}

/// Toggle button for live updates
#[component]
pub fn LiveToggle() -> impl IntoView {
    let live_state = expect_context::<LiveState>();

    let toggle_class = move || {
        if !live_state.connected.get() {
            "live-toggle live-toggle-disconnected"
        } else if live_state.enabled.get() {
            "live-toggle live-toggle-active"
        } else {
            "live-toggle live-toggle-paused"
        }
    };

    let toggle_title = move || {
        if !live_state.connected.get() {
            "Disconnected"
        } else if live_state.enabled.get() {
            "Live updates ON - click to pause"
        } else {
            "Live updates PAUSED - click to resume"
        }
    };

    let toggle_label = move || {
        if !live_state.connected.get() {
            "Offline"
        } else if live_state.enabled.get() {
            "Live"
        } else {
            "Paused"
        }
    };

    view! {
        <button
            class={toggle_class}
            on:click={move |_| live_state.toggle()}
            title={toggle_title}
        >
            <span class="live-indicator"></span>
            <span class="live-label">
                {toggle_label}
            </span>
        </button>
    }
}

/// Status display showing last update
#[component]
pub fn LiveStatus() -> impl IntoView {
    let live_state = expect_context::<LiveState>();

    view! {
        <div class="live-status">
            {move || {
                if let Some(change) = live_state.last_change.get() {
                    change.clone()
                } else {
                    "Ready".to_string()
                }
            }}
        </div>
    }
}

/// Provider component for live updates state
#[component]
pub fn LiveUpdatesProvider(
    children: Children,
) -> impl IntoView {
    let live_state = LiveState::new();
    provide_context(live_state);

    view! {
        {children()}
    }
}
