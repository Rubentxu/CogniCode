/**
 * `MermaidRenderer` — renders Mermaid diagram text as HTML.
 *
 * v1: Uses a `<pre><code>` block with mermaid syntax (no live rendering).
 * The `mermaid` library can be added later for live rendering.
 * Fallback: plain text display if rendering fails.
 *
 * ADR-008 §RendererRegistry: All renderer components are registered in
 * `rendererRegistry.tsx` and invoked via `registry.render("mermaid", body)`.
 */
import React from "react";

export interface MermaidRendererProps {
  /** Mermaid diagram definition text. */
  mermaidText: string;
  /** The ViewKind this renderer is being used for (e.g., "call_graph", "vertical_slice"). */
  viewKind?: string;
}

/**
 * Renders Mermaid text as a styled code block.
 *
 * Props accepted: `{ mermaidText?: string, viewKind?: string }`.
 */
export function MermaidRenderer({ mermaidText, viewKind }: MermaidRendererProps): React.ReactElement {
  if (!mermaidText || mermaidText.trim() === "") {
    return (
      <div
        data-testid="renderer-mermaid-empty"
        style={{
          color: "var(--color-text-muted)",
          fontFamily: "monospace",
          fontSize: "0.75rem",
          padding: "0.75rem",
        }}
      >
        No mermaid text provided.
      </div>
    );
  }

  // Detect if text looks like valid mermaid (starts with common diagram types)
  const isLikelyMermaid = /^(flowchart|graph|sequenceDiagram|classDiagram|stateDiagram|erDiagram|pie|requirementDiagram|gantt)/.test(
    mermaidText.trim(),
  );

  if (!isLikelyMermaid) {
    return (
      <div
        data-testid="renderer-mermaid-invalid"
        style={{
          color: "var(--color-text-muted)",
          fontFamily: "monospace",
          fontSize: "0.75rem",
          padding: "0.75rem",
        }}
      >
        <span>Invalid or unrecognized mermaid diagram.</span>
        <pre
          style={{
            marginTop: "0.5rem",
            padding: "0.5rem",
            backgroundColor: "var(--color-surface-overlay)",
            borderRadius: "0.25rem",
            overflow: "auto",
          }}
        >
          {mermaidText}
        </pre>
      </div>
    );
  }

  return (
    <div
      data-testid="renderer-mermaid"
      data-view-kind={viewKind}
      style={{
        width: "100%",
        overflow: "auto",
      }}
    >
      <div
        style={{
          fontFamily: "monospace",
          fontSize: "0.75rem",
          padding: "0.75rem",
          backgroundColor: "var(--color-surface-overlay)",
          borderRadius: "0.375rem",
          border: "1px solid var(--color-border)",
        }}
      >
        {/* ViewKind badge */}
        {viewKind && (
          <div
            style={{
              fontSize: "0.625rem",
              textTransform: "uppercase" as const,
              letterSpacing: "0.05em",
              color: "var(--color-text-secondary)",
              marginBottom: "0.5rem",
            }}
          >
            {viewKind}
          </div>
        )}
        <pre
          style={{
            margin: 0,
            whiteSpace: "pre-wrap" as const,
            wordBreak: "break-word" as const,
          }}
        >
          <code>{mermaidText}</code>
        </pre>
      </div>
    </div>
  );
}
