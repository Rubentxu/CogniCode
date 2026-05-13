//! DiagramViewer component — renders Mermaid diagrams client-side with zoom/pan and export
//!
//! Uses mermaid.js loaded from CDN to render diagrams via explicit mermaid.render().

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use js_sys::Promise;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize)]
pub struct SharedDiagramState {
    pub code: String,
    #[serde(default)]
    pub diagram_type: String,
    #[serde(default)]
    pub theme: String,
}

/// Check URL for shared diagram state and return it if present
pub fn get_shared_diagram_state_from_url() -> Option<SharedDiagramState> {
    let json_str = getSharedDiagramState()?;
    serde_json::from_str(&json_str).ok()
}

#[wasm_bindgen(inline_js = r#"
export function initMermaid() {
    if (window.__mermaidInitialized) return;
    window.__mermaidInitialized = true;
    const script = document.createElement('script');
    script.src = 'https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.min.js';
    script.onload = () => {
        window.mermaid.initialize({ startOnLoad: false, theme: 'default', securityLevel: 'loose' });
    };
    document.head.appendChild(script);
}

export function renderMermaidToSvg(code) {
    return new Promise((resolve, reject) => {
        if (!window.mermaid || !window.mermaid.render) {
            reject(new Error('Mermaid not loaded yet'));
            return;
        }
        const id = 'mermaid-' + Math.random().toString(36).substr(2, 9);
        window.mermaid.render(id, code).then(result => {
            resolve(result.svg);
        }).catch(err => {
            reject(err);
        });
    });
}

export function downloadSvg(svgContent, filename) {
    const blob = new Blob([svgContent], { type: 'image/svg+xml' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    a.click();
    URL.revokeObjectURL(url);
}

export function downloadPng(svgContent, filename) {
    return new Promise((resolve) => {
        const img = new Image();
        const blob = new Blob([svgContent], { type: 'image/svg+xml' });
        const url = URL.createObjectURL(blob);
        img.onload = () => {
            const canvas = document.createElement('canvas');
            canvas.width = img.width * 2;
            canvas.height = img.height * 2;
            const ctx = canvas.getContext('2d');
            ctx.scale(2, 2);
            ctx.drawImage(img, 0, 0);
            canvas.toBlob(pngBlob => {
                const pngUrl = URL.createObjectURL(pngBlob);
                const a = document.createElement('a');
                a.href = pngUrl;
                a.download = filename;
                a.click();
                URL.revokeObjectURL(pngUrl);
                URL.revokeObjectURL(url);
                resolve();
            });
        };
        img.src = url;
    });
}

export function copyToClipboard(text) {
    navigator.clipboard.writeText(text);
}

export function generateShareUrl(mermaidCode, diagramType, theme) {
    try {
        const state = {
            code: mermaidCode,
            type: diagramType || 'c4',
            theme: theme || 'default'
        };
        const encoded = btoa(unescape(encodeURIComponent(JSON.stringify(state))));
        const baseUrl = window.location.origin + window.location.pathname;
        return baseUrl + '?diagram=' + encoded;
    } catch (e) {
        console.error('Failed to generate share URL:', e);
        return null;
    }
}

export function generateEmbedSnippet(mermaidCode, diagramType, theme) {
    try {
        const state = {
            code: mermaidCode,
            type: diagramType || 'c4',
            theme: theme || 'default'
        };
        const encoded = btoa(unescape(encodeURIComponent(JSON.stringify(state))));
        const iframeSrc = window.location.origin + window.location.pathname + '?embed=' + encoded;
        return '<iframe src="' + iframeSrc + '" width="100%" height="400" frameborder="0"></iframe>';
    } catch (e) {
        console.error('Failed to generate embed snippet:', e);
        return null;
    }
}

export function parseShareUrl() {
    try {
        const params = new URLSearchParams(window.location.search);
        const diagramParam = params.get('diagram') || params.get('embed');
        if (diagramParam) {
            const decoded = decodeURIComponent(escape(atob(diagramParam)));
            return JSON.parse(decoded);
        }
        return null;
    } catch (e) {
        console.error('Failed to parse share URL:', e);
        return null;
    }
}

// Returns share state as JSON string for Rust consumption
export function getSharedDiagramState() {
    try {
        const state = parseShareUrl();
        if (state && state.code) {
            return JSON.stringify({
                code: state.code || '',
                diagram_type: state.type || 'c4',
                theme: state.theme || 'default'
            });
        }
        return null;
    } catch (e) {
        console.error('Failed to get shared diagram state:', e);
        return null;
    }
}

// Set up click-to-scroll for diff-highlighted elements
export function setupDiffClickScroll(containerId) {
    // Find the container and set up click handlers for diff-highlighted elements
    const container = document.getElementById(containerId);
    if (!container) return;

    // Use MutationObserver to watch for SVG changes (Mermaid re-renders)
    const observer = new MutationObserver((mutations) => {
        mutations.forEach((mutation) => {
            if (mutation.type === 'childList' || mutation.type === 'subtree') {
                attachClickHandlers();
            }
        });
    });

    observer.observe(container, { childList: true, subtree: true });

    // Initial attachment after a short delay to allow Mermaid to render
    setTimeout(attachClickHandlers, 100);
    setTimeout(attachClickHandlers, 500);
    setTimeout(attachClickHandlers, 1000);

    function attachClickHandlers() {
        // Find all diff-highlighted elements (groups, rects, circles, polygons with diff classes)
        const diffSelectors = [
            '.diff-added',
            '.diff-removed',
            '.diff-modified',
            '[class*="diff-added"]',
            '[class*="diff-removed"]',
            '[class*="diff-modified"]',
            '[class*="diff_"]'
        ];

        diffSelectors.forEach(selector => {
            try {
                const elements = container.querySelectorAll(selector);
                elements.forEach(el => {
                    if (el.dataset.clickScrollAttached) return;
                    el.dataset.clickScrollAttached = 'true';
                    el.style.cursor = 'pointer';

                    el.addEventListener('click', (event) => {
                        event.preventDefault();
                        event.stopPropagation();

                        // Get element position
                        const rect = el.getBoundingClientRect();
                        const svg = el.closest('svg');
                        if (!svg) return;

                        const svgRect = svg.getBoundingClientRect();

                        // Calculate center position relative to the scrollable container
                        const scrollContainer = container.closest('.diagram-container') || container;
                        const elementCenterX = rect.left + rect.width / 2 - svgRect.left;
                        const elementCenterY = rect.top + rect.height / 2 - svgRect.top;

                        // Add pulse animation
                        el.classList.add('diff-pulse');
                        setTimeout(() => el.classList.remove('diff-pulse'), 600);

                        // Scroll to center the element in both panels
                        scrollContainer.scrollTo({
                            left: elementCenterX - scrollContainer.clientWidth / 2,
                            top: elementCenterY - scrollContainer.clientHeight / 2,
                            behavior: 'smooth'
                        });

                        // Also notify the other panel via custom event
                        window.dispatchEvent(new CustomEvent('diff-element-clicked', {
                            detail: {
                                elementId: el.id || el.getAttribute('id') || '',
                                className: el.className || '',
                                centerX: elementCenterX,
                                centerY: elementCenterY
                            }
                        }));
                    });
                });
            } catch (e) {
                // Ignore invalid selectors
            }
        });
    }
}

// Scroll to a specific diff element by ID
export function scrollToDiffElement(elementId) {
    // Find element by ID in any diagram container and scroll to it
    const containers = document.querySelectorAll('.diagram-container');
    containers.forEach(container => {
        const el = container.querySelector(`#${elementId}, [id="${elementId}"]`);
        if (el) {
            const rect = el.getBoundingClientRect();
            const svg = el.closest('svg');
            if (!svg) return;

            const svgRect = svg.getBoundingClientRect();
            const scrollContainer = container.closest('.diagram-container') || container;

            const elementCenterX = rect.left + rect.width / 2 - svgRect.left;
            const elementCenterY = rect.top + rect.height / 2 - svgRect.top;

            el.classList.add('diff-pulse');
            setTimeout(() => el.classList.remove('diff-pulse'), 600);

            scrollContainer.scrollTo({
                left: elementCenterX - scrollContainer.clientWidth / 2,
                top: elementCenterY - scrollContainer.clientHeight / 2,
                behavior: 'smooth'
            });
        }
    });
}
"#)]
extern "C" {
    fn initMermaid();
    fn renderMermaidToSvg(code: &str) -> Promise;
    fn downloadSvg(svgContent: &str, filename: &str);
    fn downloadPng(svgContent: &str, filename: &str) -> Promise;
    fn copyToClipboard(text: &str);
    fn generateShareUrl(mermaidCode: &str, diagramType: &str, theme: &str) -> Option<String>;
    fn generateEmbedSnippet(mermaidCode: &str, diagramType: &str, theme: &str) -> Option<String>;
    fn getSharedDiagramState() -> Option<String>;
}

/// A component that renders Mermaid diagrams client-side with zoom/pan and export.
#[component]
pub fn DiagramViewer(
    mermaid_code: String,
    #[prop(default = false)] show_toolbar: bool,
    #[prop(default = "c4".to_string())] diagram_type: String,
) -> impl IntoView {
    let (zoom_level, set_zoom) = signal(1.0f64);
    let (pan_x, set_pan_x) = signal(0.0f64);
    let (pan_y, set_pan_y) = signal(0.0f64);
    let (fullscreen, set_fullscreen) = signal(false);
    let (copied, set_copied) = signal(false);
    let (loading, set_loading) = signal(false);
    let (error_msg, set_error_msg) = signal(None::<String>);
    let (rendered_svg, set_rendered_svg) = signal(None::<String>);
    let (show_share_modal, set_show_share_modal) = signal(false);
    let (share_url, set_share_url) = signal(None::<String>);
    let (embed_snippet, set_embed_snippet) = signal(None::<String>);
    let (copied_share, set_copied_share) = signal(false);
    let (copied_embed, set_copied_embed) = signal(false);

    // Container ref for DOM access
    let container_ref = NodeRef::<leptos::html::Div>::new();

    // Store mermaid_code in a signal so multiple closures can read it
    let (mermaid_signal, _set_mermaid) = signal(mermaid_code);

    // Initialize mermaid on mount
    Effect::new(move |_| {
        initMermaid();
    });

    // Re-render when mermaid_code changes
    Effect::new(move |_| {
        let code = mermaid_signal.get();
        let container = container_ref.get();

        if code.is_empty() {
            set_rendered_svg.set(None);
            set_error_msg.set(None);
            return;
        }

        set_loading.set(true);
        set_error_msg.set(None);

        let promise = renderMermaidToSvg(&code);
        let future = JsFuture::from(promise);

        spawn_local(async move {
            match future.await {
                Ok(svg) => {
                    let svg_str = svg.as_string().unwrap_or_default();
                    set_rendered_svg.set(Some(svg_str.clone()));
                    set_loading.set(false);

                    // Apply SVG to container
                    if let Some(cont) = container {
                        cont.set_inner_html(&svg_str);
                    }
                }
                Err(e) => {
                    let err_str = format!("{:?}", e);
                    set_error_msg.set(Some(err_str.clone()));
                    set_loading.set(false);

                    // Show raw code with error banner in container
                    if let Some(cont) = container {
                        let error_display = format!(
                            r#"<div style="color: #dc2626; background: #fef2f2; border: 1px solid #fca5a5; border-radius: 8px; padding: 16px; margin: 8px 0;">
                                <strong>Render Error:</strong>
                                <pre style="margin: 8px 0 0 0; white-space: pre-wrap; font-size: 12px;">{}</pre>
                            </div>
                            <details style="margin-top: 16px;">
                                <summary style="cursor: pointer; color: #6b7280;">Raw Mermaid Code</summary>
                                <pre style="background: #f9fafb; padding: 12px; border-radius: 6px; margin-top: 8px; font-size: 12px; overflow-x: auto;">{}</pre>
                            </details>"#,
                            err_str, code
                        );
                        cont.set_inner_html(&error_display);
                    }
                }
            }
        });
    });

    view! {
        <div
            class="diagram-viewer"
            style="
                background: var(--color-surface-raised);
                border: 1px solid var(--color-border);
                border-radius: 12px;
                box-shadow: var(--shadow-card);
                overflow: hidden;
            "
        >
            {/* Toolbar */}
            <div style="
                display: flex;
                align-items: center;
                justify-content: space-between;
                padding: 12px 16px;
                border-bottom: 1px solid var(--color-border);
                background: var(--color-surface);
            ">
                <span style="font-size: 13px; font-weight: 600; color: var(--color-text-secondary);">
                    "Mermaid Diagram"
                </span>

                <div style="display: flex; gap: 8px; align-items: center;">
                    {/* Zoom controls */}
                    <button
                        on:click={move |_| { set_zoom.update(|z| { *z = (*z / 1.2).max(0.2); }); }}
                        title="Zoom Out"
                        style="
                            display: flex;
                            align-items: center;
                            justify-content: center;
                            width: 32px;
                            height: 32px;
                            padding: 0;
                            font-size: 16px;
                            font-weight: 600;
                            color: var(--color-text-secondary);
                            background: var(--color-surface-raised);
                            border: 1px solid var(--color-border);
                            border-radius: 6px;
                            cursor: pointer;
                        "
                    >
                        "−"
                    </button>

                    <span style="font-size: 11px; color: var(--color-text-muted); min-width: 40px; text-align: center;">
                        {move || format!("{}%", (zoom_level.get() * 100.0) as i32)}
                    </span>

                    <button
                        on:click={move |_| { set_zoom.update(|z| { *z = (*z * 1.2).min(5.0); }); }}
                        title="Zoom In"
                        style="
                            display: flex;
                            align-items: center;
                            justify-content: center;
                            width: 32px;
                            height: 32px;
                            padding: 0;
                            font-size: 16px;
                            font-weight: 600;
                            color: var(--color-text-secondary);
                            background: var(--color-surface-raised);
                            border: 1px solid var(--color-border);
                            border-radius: 6px;
                            cursor: pointer;
                        "
                    >
                        "+"
                    </button>

                    <button
                        on:click={move |_| { set_zoom.set(1.0); set_pan_x.set(0.0); set_pan_y.set(0.0); }}
                        title="Reset Zoom"
                        style="
                            display: flex;
                            align-items: center;
                            justify-content: center;
                            width: 32px;
                            height: 32px;
                            padding: 0;
                            font-size: 14px;
                            color: var(--color-text-secondary);
                            background: var(--color-surface-raised);
                            border: 1px solid var(--color-border);
                            border-radius: 6px;
                            cursor: pointer;
                        "
                    >
                        <svg style="width: 14px; height: 14px;" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"/>
                        </svg>
                    </button>

                    <div style="width: 1px; height: 20px; background: var(--color-border); margin: 0 4px;"></div>

                    {/* Copy button */}
                    <button
                        on:click={let code = mermaid_signal.get(); move |_| {
                            copyToClipboard(&code);
                            set_copied.set(true);
                        }}
                        title="Copy Mermaid Code"
                        style="
                            display: flex;
                            align-items: center;
                            gap: 6px;
                            padding: 6px 12px;
                            font-size: 12px;
                            font-weight: 500;
                            color: var(--color-text-secondary);
                            background: var(--color-surface-raised);
                            border: 1px solid var(--color-border);
                            border-radius: 6px;
                            cursor: pointer;
                        "
                    >
                        <Show when={move || copied.get()}>
                            <span style="color: #16a34a;">"✓ Copied"</span>
                        </Show>
                        <Show when={move || !copied.get()}>
                            <svg style="width: 14px; height: 14px;" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"/>
                            </svg>
                            "Copy"
                        </Show>
                    </button>

                    {/* Download SVG */}
                    <button
                        on:click={move |_| {
                            if let Some(svg_content) = rendered_svg.get() {
                                let filename = format!("diagram-{}.svg", chrono::Utc::now().format("%Y%m%d-%H%M%S"));
                                downloadSvg(&svg_content, &filename);
                            }
                        }}
                        title="Download SVG"
                        style="
                            display: flex;
                            align-items: center;
                            gap: 6px;
                            padding: 6px 12px;
                            font-size: 12px;
                            font-weight: 500;
                            color: var(--color-text-secondary);
                            background: var(--color-surface-raised);
                            border: 1px solid var(--color-border);
                            border-radius: 6px;
                            cursor: pointer;
                        "
                    >
                        <svg style="width: 14px; height: 14px;" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4"/>
                        </svg>
                        "SVG"
                    </button>

                    {/* Download PNG */}
                    <button
                        on:click={move |_| {
                            if let Some(svg_content) = rendered_svg.get() {
                                let filename = format!("diagram-{}.png", chrono::Utc::now().format("%Y%m%d-%H%M%S"));
                                let svg_clone = svg_content.clone();
                                spawn_local(async move {
                                    downloadPng(&svg_clone, &filename).await;
                                });
                            }
                        }}
                        title="Download PNG"
                        style="
                            display: flex;
                            align-items: center;
                            gap: 6px;
                            padding: 6px 12px;
                            font-size: 12px;
                            font-weight: 500;
                            color: var(--color-text-secondary);
                            background: var(--color-surface-raised);
                            border: 1px solid var(--color-border);
                            border-radius: 6px;
                            cursor: pointer;
                        "
                    >
                        <svg style="width: 14px; height: 14px;" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4"/>
                        </svg>
                        "PNG"
                    </button>

                    {/* Share button */}
                    <button
                        on:click={move |_| {
                            let code = mermaid_signal.get();
                            if let Some(url) = generateShareUrl(&code, &diagram_type, "default") {
                                set_share_url.set(Some(url));
                                set_embed_snippet.set(generateEmbedSnippet(&code, &diagram_type, "default"));
                                set_show_share_modal.set(true);
                            }
                        }}
                        title="Share Diagram"
                        style="
                            display: flex;
                            align-items: center;
                            gap: 6px;
                            padding: 6px 12px;
                            font-size: 12px;
                            font-weight: 500;
                            color: var(--color-text-secondary);
                            background: var(--color-surface-raised);
                            border: 1px solid var(--color-border);
                            border-radius: 6px;
                            cursor: pointer;
                        "
                    >
                        <svg style="width: 14px; height: 14px;" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M7.217 10.907a2.25 2.25 0 100 2.186m0-2.186c.18.324.283.696.283 1.093s-.103.77-.283 1.093m0-2.186l9.566-5.314m-9.566 7.5l9.566 5.314m0 0a2.25 2.25 0 103.935 2.186 2.25 2.25 0 00-3.935-2.186zm0-12.814a2.25 2.25 0 103.933-2.185 2.25 2.25 0 00-3.933 2.185z"/>
                        </svg>
                        "Share"
                    </button>

                    {/* Fullscreen toggle */}
                    <button
                        on:click={move |_| { set_fullscreen.update(|v| { *v = !*v; }); }}
                        title="Fullscreen"
                        style="
                            display: flex;
                            align-items: center;
                            justify-content: center;
                            width: 32px;
                            height: 32px;
                            padding: 0;
                            font-size: 14px;
                            color: var(--color-text-secondary);
                            background: var(--color-surface-raised);
                            border: 1px solid var(--color-border);
                            border-radius: 6px;
                            cursor: pointer;
                        "
                    >
                        <svg style="width: 14px; height: 14px;" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M3.75 3.75v4.5m0-4.5h4.5m-4.5 0L9 9M3.75 20.25v-4.5m0 4.5h4.5m-4.5 0L9 15M20.25 3.75h-4.5m4.5 0v4.5m0-4.5L15 9m5.25 11.25h-4.5m4.5 0v-4.5m0 4.5L15 15"/>
                        </svg>
                    </button>
                </div>
            </div>

            {/* Diagram area */}
            <div
                node_ref=container_ref
                class="diagram-container"
                style="
                    padding: 24px;
                    overflow: auto;
                    min-height: 300px;
                "
            >
                {/* Loading spinner */}
                <Show when={move || loading.get()}>
                    <div style="
                        display: flex;
                        align-items: center;
                        justify-content: center;
                        min-height: 200px;
                    ">
                        <div style="
                            width: 40px;
                            height: 40px;
                            border: 3px solid var(--color-border);
                            border-top-color: var(--color-accent);
                            border-radius: 50%;
                            animation: spin 1s linear infinite;
                        "></div>
                    </div>
                </Show>

                {/* Empty state */}
                <Show when={move || !loading.get() && rendered_svg.get().is_none() && error_msg.get().is_none() && mermaid_signal.get().is_empty()}>
                    <div style="
                        display: flex;
                        flex-direction: column;
                        align-items: center;
                        justify-content: center;
                        min-height: 200px;
                        color: var(--color-text-muted);
                    ">
                        <svg style="width: 48px; height: 48px; margin-bottom: 16px; opacity: 0.5;" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M9 17.25v1.007a3 3 0 01-.879 2.122L7.5 21h9l-.621-.621A3 3 0 0115 18.257V17.25m6-12V15a2.25 2.25 0 01-2.25 2.25H5.25A2.25 2.25 0 013 15V5.25m18 0A2.25 2.25 0 0018.75 3H5.25A2.25 2.25 0 003 5.25m18 0V12a2.25 2.25 0 01-2.25 2.25H5.25A2.25 2.25 0 003 12V5.25"/>
                        </svg>
                        <span style="font-size: 14px;">"No diagram to display"</span>
                    </div>
                </Show>
            </div>
        </div>

        {/* Share Modal */}
        <Show when={move || show_share_modal.get()}>
            <div style="
                position: fixed;
                top: 0;
                left: 0;
                right: 0;
                bottom: 0;
                background: rgba(0, 0, 0, 0.5);
                display: flex;
                align-items: center;
                justify-content: center;
                z-index: 1000;
            " on:click={move |_| set_show_share_modal.set(false)}>
                <div style="
                    background: var(--color-surface-raised);
                    border: 1px solid var(--color-border);
                    border-radius: 12px;
                    padding: 24px;
                    max-width: 500px;
                    width: 90%;
                    box-shadow: var(--shadow-card);
                " on:click=|ev: web_sys::MouseEvent| ev.stop_propagation()>
                    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 20px;">
                        <h3 style="font-size: 18px; font-weight: 600; color: var(--color-text-primary);">"Share Diagram"</h3>
                        <button
                            on:click={move |_| set_show_share_modal.set(false)}
                            style="
                                background: none;
                                border: none;
                                font-size: 20px;
                                cursor: pointer;
                                color: var(--color-text-muted);
                                padding: 4px 8px;
                            "
                        >
                            "×"
                        </button>
                    </div>

                    {/* Share URL */}
                    <div style="margin-bottom: 20px;">
                        <label style="display: block; font-size: 13px; font-weight: 500; color: var(--color-text-secondary); margin-bottom: 8px;">
                            "Shareable URL"
                        </label>
                        <div style="display: flex; gap: 8px;">
                            <input
                                type="text"
                                readonly
                                value={share_url.get().unwrap_or_default()}
                                style="
                                    flex: 1;
                                    padding: 10px 12px;
                                    font-size: 13px;
                                    font-family: monospace;
                                    color: var(--color-text-primary);
                                    background: var(--color-surface);
                                    border: 1px solid var(--color-border);
                                    border-radius: 6px;
                                    outline: none;
                                "
                            />
                            <button
                                on:click={move |_| {
                                    if let Some(url) = share_url.get() {
                                        copyToClipboard(&url);
                                        set_copied_share.set(true);
                                        set_timeout(move || set_copied_share.set(false), Duration::from_millis(2000));
                                    }
                                }}
                                style="
                                    padding: 10px 16px;
                                    font-size: 12px;
                                    font-weight: 500;
                                    color: #ffffff;
                                    background: var(--color-accent-sky);
                                    border: none;
                                    border-radius: 6px;
                                    cursor: pointer;
                                "
                            >
                                {move || if copied_share.get() { "Copied!" } else { "Copy" }}
                            </button>
                        </div>
                    </div>

                    {/* Embed Snippet */}
                    <div>
                        <label style="display: block; font-size: 13px; font-weight: 500; color: var(--color-text-secondary); margin-bottom: 8px;">
                            "Embed Code (iframe)"
                        </label>
                        <div style="display: flex; gap: 8px;">
                            <pre
                                style="
                                    flex: 1;
                                    padding: 10px 12px;
                                    font-size: 12px;
                                    font-family: monospace;
                                    color: var(--color-text-primary);
                                    background: var(--color-surface);
                                    border: 1px solid var(--color-border);
                                    border-radius: 6px;
                                    outline: none;
                                    overflow-x: auto;
                                    margin: 0;
                                "
                            >{embed_snippet.get().unwrap_or_default()}</pre>
                            <button
                                on:click={move |_| {
                                    if let Some(snippet) = embed_snippet.get() {
                                        copyToClipboard(&snippet);
                                        set_copied_embed.set(true);
                                        set_timeout(move || set_copied_embed.set(false), Duration::from_millis(2000));
                                    }
                                }}
                                style="
                                    padding: 10px 16px;
                                    font-size: 12px;
                                    font-weight: 500;
                                    color: #ffffff;
                                    background: var(--color-accent-sky);
                                    border: none;
                                    border-radius: 6px;
                                    cursor: pointer;
                                    align-self: flex-start;
                                "
                            >
                                {move || if copied_embed.get() { "Copied!" } else { "Copy" }}
                            </button>
                        </div>
                    </div>
                </div>
            </div>
        </Show>

        <style>
            {r#"
                @keyframes spin {
                    to { transform: rotate(360deg); }
                }
                .diagram-viewer svg,
                .diagram-container svg {
                    max-width: 100%;
                    height: auto;
                }
            "#}
        </style>
    }
}
