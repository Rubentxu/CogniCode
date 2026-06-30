/**
 * ExportMenu — dropdown menu for exporting view content.
 *
 * Provides "Open in draw.io" option that extracts Mermaid text from
 * the current view's blocks and opens it in draw.io.
 * Also provides "Download PNG" and "Download SVG" options that call
 * the snapshot API and trigger a browser download.
 */
import { useState, useRef, useEffect } from "react";
import type { ContextualView } from "../api/types";
import { fetchSnapshot } from "../api/client";
import { handleOpenInDrawIo } from "../utils/drawio";
import { downloadSnapshot } from "../utils/download";

export interface ExportMenuProps {
  view: ContextualView | null;
  workspaceId: string;
  onShowNotification?: (message: string) => void;
}

/**
 * Extract Mermaid text from view blocks.
 * Looks for blocks with body.mermaidText property.
 */
function extractMermaidFromView(view: ContextualView | null): string | null {
  if (!view) return null;
  for (const block of view.blocks) {
    const body = block.body as { mermaidText?: string } | null;
    if (body?.mermaidText) {
      return body.mermaidText;
    }
  }
  return null;
}

export function ExportMenu({ view, workspaceId, onShowNotification }: ExportMenuProps) {
  const [isOpen, setIsOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  // Close menu when clicking outside
  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    }
    if (isOpen) {
      document.addEventListener("mousedown", handleClickOutside);
      return () => document.removeEventListener("mousedown", handleClickOutside);
    }
  }, [isOpen]);

  const handleOpenInDrawIoAction = async () => {
    setIsOpen(false);
    const mermaidText = extractMermaidFromView(view);
    if (!mermaidText) {
      onShowNotification?.("No Mermaid diagram available in this view");
      return;
    }
    try {
      await handleOpenInDrawIo(mermaidText, { notify: onShowNotification });
    } catch {
      onShowNotification?.("Failed to open diagram in draw.io");
    }
  };

  const handleDownloadSnapshot = async (format: "png" | "svg") => {
    setIsOpen(false);
    if (!view?.view_kind) {
      onShowNotification?.("Cannot download: view kind not available");
      return;
    }
    // The object_id is the target for the snapshot API
    const target = view.object_id;
    try {
      const blob = await fetchSnapshot(workspaceId, view.view_kind, format, target);
      const extension = format === "png" ? ".png" : ".svg";
      const filename = `diagram${extension}`;
      await downloadSnapshot(blob, filename);
      onShowNotification?.(`Downloaded ${format.toUpperCase()}`);
    } catch {
      onShowNotification?.(`Failed to download ${format.toUpperCase()}`);
    }
  };

  return (
    <div ref={menuRef} style={{ position: "relative" }}>
      <button
        type="button"
        aria-label="Export options"
        aria-expanded={isOpen}
        aria-haspopup="menu"
        data-testid="export-menu-trigger"
        onClick={() => setIsOpen(!isOpen)}
        style={{
          display: "flex",
          alignItems: "center",
          gap: 4,
          padding: "4px 8px",
          borderRadius: 4,
          border: "1px solid var(--color-border)",
          backgroundColor: "var(--color-surface-overlay)",
          color: "var(--color-text-secondary)",
          fontSize: 12,
          cursor: "pointer",
        }}
      >
        <svg
          width="12"
          height="12"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
          <polyline points="7 10 12 15 17 10" />
          <line x1="12" y1="15" x2="12" y2="3" />
        </svg>
        Export
        <svg
          width="10"
          height="10"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <polyline points="6 9 12 15 18 9" />
        </svg>
      </button>

      {isOpen && (
        <div
          role="menu"
          aria-label="Export options"
          data-testid="export-menu-dropdown"
          style={{
            position: "absolute",
            right: 0,
            top: "100%",
            marginTop: 4,
            minWidth: 180,
            backgroundColor: "var(--color-surface)",
            border: "1px solid var(--color-border)",
            borderRadius: 6,
            boxShadow: "0 4px 12px rgba(0,0,0,0.15)",
            zIndex: 100,
          }}
        >
          <button
            type="button"
            role="menuitem"
            data-testid="export-menu-open-drawio"
            onClick={handleOpenInDrawIoAction}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 8,
              width: "100%",
              padding: "8px 12px",
              border: "none",
              backgroundColor: "transparent",
              color: "var(--color-text-primary)",
              fontSize: 13,
              cursor: "pointer",
              textAlign: "left",
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.backgroundColor = "var(--color-surface-overlay)";
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.backgroundColor = "transparent";
            }}
          >
            <svg
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
              aria-hidden="true"
            >
              <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
              <polyline points="14 2 14 8 20 8" />
            </svg>
            Open in draw.io
          </button>
          <button
            type="button"
            role="menuitem"
            data-testid="export-menu-download-png"
            onClick={() => handleDownloadSnapshot("png")}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 8,
              width: "100%",
              padding: "8px 12px",
              border: "none",
              backgroundColor: "transparent",
              color: "var(--color-text-primary)",
              fontSize: 13,
              cursor: "pointer",
              textAlign: "left",
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.backgroundColor = "var(--color-surface-overlay)";
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.backgroundColor = "transparent";
            }}
          >
            <svg
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
              aria-hidden="true"
            >
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
              <polyline points="7 10 12 15 17 10" />
              <line x1="12" y1="15" x2="12" y2="3" />
            </svg>
            Download as PNG
          </button>
          <button
            type="button"
            role="menuitem"
            data-testid="export-menu-download-svg"
            onClick={() => handleDownloadSnapshot("svg")}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 8,
              width: "100%",
              padding: "8px 12px",
              border: "none",
              backgroundColor: "transparent",
              color: "var(--color-text-primary)",
              fontSize: 13,
              cursor: "pointer",
              textAlign: "left",
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.backgroundColor = "var(--color-surface-overlay)";
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.backgroundColor = "transparent";
            }}
          >
            <svg
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
              aria-hidden="true"
            >
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
              <polyline points="7 10 12 15 17 10" />
              <line x1="12" y1="15" x2="12" y2="3" />
            </svg>
            Download as SVG
          </button>
        </div>
      )}
    </div>
  );
}
