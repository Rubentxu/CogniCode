//! Shell layout component with sidebar navigation
//! Responsive: sidebar collapses to hamburger menu on mobile (< 768px)

use leptos::prelude::*;

#[derive(Clone, Debug)]
pub struct NavItem {
    pub label: &'static str,
    pub href: &'static str,
    pub icon: &'static str,
}

impl NavItem {
    pub const fn new(label: &'static str, href: &'static str, icon: &'static str) -> Self {
        Self { label, href, icon }
    }
}

const NAV_ITEMS: &[NavItem] = &[
    NavItem::new("Projects", "/projects", "M3 7v2a1 1 0 001 1h2a1 1 0 001-1V7a1 1 0 00-1-1H4a1 1 0 00-1 1v2a1 1 0 001 1h2a1 1 0 001-1z M4 4h16M4 4v16M4 4L20 20"),
    NavItem::new("Dashboard", "/", "M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6"),
    NavItem::new("Issues", "/issues", "M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2m-3 7h3m-3 4h3m-6-4h.01M9 16h.01"),
    NavItem::new("Metrics", "/metrics", "M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z"),
    NavItem::new("Quality Gate", "/quality-gate", "M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"),
    NavItem::new("Configuration", "/configuration", "M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z M15 12a3 3 0 11-6 0 3 3 0 016 0z"),
];

/// Navigation sidebar with responsive hamburger menu
#[component]
pub fn Shell(children: Children) -> impl IntoView {
    let (sidebar_open, set_sidebar_open) = signal(false);
    let (dark_mode, set_dark_mode) = signal(false);

    let on_toggle = move || {
        set_dark_mode.update(|v| *v = !*v);
    };

    view! {
        <div class="app-shell" style="display: flex; min-height: 100vh;">
            {/* Injected style for dark mode toggling */}
            <style>
                {move || if dark_mode.get() {
                    r#"
                    :root {
                        --color-canvas: #0f172a !important;
                        --color-surface: #1e293b !important;
                        --color-surface-raised: #1e293b !important;
                        --color-text-primary: #f1f5f9 !important;
                        --color-text-secondary: #94a3b8 !important;
                        --color-text-muted: #64748b !important;
                        --color-border: #334155 !important;
                        --color-border-strong: #475569 !important;
                        --color-accent-pale: rgba(97, 97, 255, 0.15) !important;
                        --color-accent-ocean: rgba(34, 197, 94, 0.15) !important;
                        --color-accent-sky: rgba(59, 130, 246, 0.15) !important;
                        --color-accent-sunset: rgba(239, 68, 68, 0.15) !important;
                        --shadow-card: 0 2px 8px 0 rgba(0, 0, 0, 0.3) !important;
                        --shadow-elevated: 0 4px 16px 0 rgba(0, 0, 0, 0.4) !important;
                    }
                    "#
                } else {
                    r#"
                    :root {
                        --color-canvas: #ffffff;
                        --color-surface: #f8f9fc;
                        --color-surface-raised: #ffffff;
                        --color-text-primary: #111827;
                        --color-text-secondary: #4b5563;
                        --color-text-muted: #9ca3af;
                        --color-border: #e5e7eb;
                        --color-border-strong: #d1d5db;
                        --color-accent-pale: #eef2ff;
                        --color-accent-ocean: #d1fae5;
                        --color-accent-sky: #dbeafe;
                        --color-accent-sunset: #fee2e2;
                        --shadow-card: 0 2px 8px 0 rgba(205, 208, 223, 0.4);
                        --shadow-elevated: 0 4px 16px 0 rgba(205, 208, 223, 0.5);
                    }
                    "#
                }}
            </style>

            {/* Mobile hamburger button */}
            <button
                class="hamburger-btn"
                on:click=move |_| set_sidebar_open.update(|v| *v = !*v)
                style="display: none; position: fixed; top: 12px; left: 12px; z-index: 200; background: var(--color-surface-raised); border: 1px solid var(--color-border); border-radius: 8px; padding: 8px; cursor: pointer;"
                aria-label="Toggle menu"
            >
                <svg style="width: 24px; height: 24px; color: var(--color-text-primary);" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M4 6h16M4 12h16M4 18h16"/>
                </svg>
            </button>

            {/* Mobile overlay */}
            <Show when={move || sidebar_open.get()}>
                <div
                    class="sidebar-overlay"
                    on:click=move |_| set_sidebar_open.update(|v| *v = !*v)
                    style="display: none; position: fixed; inset: 0; background: rgba(0, 0, 0, 0.5); z-index: 90;"
                />
            </Show>

            {/* Desktop sidebar */}
            <aside class="sidebar-desktop" style="position: fixed; left: 0; top: 0; bottom: 0; width: var(--sidebar-width); background: var(--color-surface-raised); border-right: 1px solid var(--color-border); display: flex; flex-direction: column; z-index: 100;">
                <SidebarContent dark_mode=dark_mode on_toggle_dark=on_toggle.clone() />
            </aside>

            {/* Mobile sidebar (slide-in) */}
            <aside
                class="sidebar-mobile"
                style="position: fixed; left: 0; top: 0; bottom: 0; width: var(--sidebar-width); background: var(--color-surface-raised); border-right: 1px solid var(--color-border); display: flex; flex-direction: column; z-index: 150; transition: transform 0.3s ease;"
                style:transform={if sidebar_open.get() { "translateX(0)" } else { "translateX(-100%)" }}
            >
                <SidebarContent dark_mode=dark_mode on_toggle_dark=on_toggle />
            </aside>

            <main class="main-content" style="flex: 1; padding: 32px; min-height: 100vh;">
                {children()}
            </main>
        </div>

        {/* Responsive CSS */}
        <style>
            {r#"
                @media (max-width: 768px) {
                    .sidebar-desktop {
                        display: none !important;
                    }
                    .hamburger-btn {
                        display: block !important;
                    }
                    .main-content {
                        margin-left: 0 !important;
                        padding-top: 64px !important;
                    }
                    .sidebar-overlay {
                        display: block !important;
                    }
                    .sidebar-mobile {
                        transform: translateX(-100%);
                    }
                    .sidebar-mobile[style*="translateX(0)"] {
                        transform: translateX(0) !important;
                    }
                }
                @media (min-width: 769px) {
                    .sidebar-mobile {
                        display: none !important;
                    }
                    .main-content {
                        margin-left: var(--sidebar-width);
                    }
                }
            "#}
        </style>
    }
}

/// Sidebar content shared between desktop and mobile
#[component]
fn SidebarContent(dark_mode: ReadSignal<bool>, on_toggle_dark: impl Fn() + 'static) -> impl IntoView {
    view! {
        <div style="padding: 24px; border-bottom: 1px solid var(--color-border);">
            <div style="display: flex; align-items: center; gap: 16px;">
                <svg style="width: 32px; height: 32px; color: var(--color-brand);" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"/>
                </svg>
                <div style="display: flex; flex-direction: column;">
                    <span style="font-size: 16px; font-weight: 700; color: var(--color-text-primary); letter-spacing: -0.01em;">CogniCode</span>
                    <span style="font-size: 12px; color: var(--color-text-muted);">v0.1.0</span>
                </div>
            </div>
        </div>

        <nav style="flex: 1; padding: 8px; display: flex; flex-direction: column; gap: 4px;">
            {NAV_ITEMS.iter().map(|item| {
                let href = item.href.to_string();
                view! {
                    <a href={href} style="display: flex; align-items: center; gap: 16px; padding: 12px 16px; border-radius: 8px; text-decoration: none; color: var(--color-text-secondary); font-size: 14px; font-weight: 500; transition: all 0.15s ease;">
                        <svg style="width: 20px; height: 20px; flex-shrink: 0;" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                            <path stroke-linecap="round" stroke-linejoin="round" d={item.icon}/>
                        </svg>
                        <span style="white-space: nowrap;">{item.label}</span>
                    </a>
                }
            }).collect::<Vec<_>>()}
        </nav>

        <div style="padding: 24px; border-top: 1px solid var(--color-border); display: flex; flex-direction: column; gap: 16px;">
            {/* Dark Mode Toggle */}
            <button
                on:click=move |_| on_toggle_dark()
                style="display: flex; align-items: center; gap: 12px; width: 100%; padding: 8px 12px; border-radius: 8px; border: none; background: var(--color-surface); cursor: pointer; color: var(--color-text-secondary); font-size: 13px; font-weight: 500; transition: all 0.15s ease;"
            >
                <svg style="width: 18px; height: 18px; flex-shrink: 0;" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    {move || if dark_mode.get() {
                        view! { <path stroke-linecap="round" stroke-linejoin="round" d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z"/> }
                    } else {
                        view! { <path stroke-linecap="round" stroke-linejoin="round" d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z"/> }
                    }}
                </svg>
                {move || if dark_mode.get() { "Light Mode" } else { "Dark Mode" }}
            </button>

            <div style="text-align: center;">
                <span style="font-size: 12px; color: var(--color-text-muted); text-transform: uppercase; letter-spacing: 0.05em;">Quality Dashboard</span>
            </div>
        </div>
    }
}