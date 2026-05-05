//! Shell layout component with sidebar navigation

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
    NavItem::new("Dashboard", "/", "M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6"),
    NavItem::new("Issues", "/issues", "M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2m-3 7h3m-3 4h3m-6-4h.01M9 16h.01"),
    NavItem::new("Metrics", "/metrics", "M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z"),
    NavItem::new("Quality Gate", "/quality-gate", "M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"),
    NavItem::new("Configuration", "/configuration", "M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z M15 12a3 3 0 11-6 0 3 3 0 016 0z"),
];

#[component]
pub fn Shell(children: Children) -> impl IntoView {
    view! {
        <div class="app-shell" style="display: flex; min-height: 100vh;">
            <aside style="position: fixed; left: 0; top: 0; bottom: 0; width: var(--sidebar-width); background: var(--color-surface-raised); border-right: 1px solid var(--color-border); display: flex; flex-direction: column; z-index: 100;">
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
                        view! {
                            <a href={item.href} style="display: flex; align-items: center; gap: 16px; padding: 12px 16px; border-radius: 8px; text-decoration: none; color: var(--color-text-secondary); font-size: 14px; font-weight: 500; transition: all 0.15s ease;">
                                <svg style="width: 20px; height: 20px; flex-shrink: 0;" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                                    <path stroke-linecap="round" stroke-linejoin="round" d={item.icon}/>
                                </svg>
                                <span style="white-space: nowrap;">{item.label}</span>
                            </a>
                        }
                    }).collect::<Vec<_>>()}
                </nav>

                <div style="padding: 24px; border-top: 1px solid var(--color-border);">
                    <div style="text-align: center;">
                        <span style="font-size: 12px; color: var(--color-text-muted); text-transform: uppercase; letter-spacing: 0.05em;">Quality Dashboard</span>
                    </div>
                </div>
            </aside>

            <main style="flex: 1; margin-left: var(--sidebar-width); padding: 32px; min-height: 100vh;">
                {children()}
            </main>
        </div>
    }
}