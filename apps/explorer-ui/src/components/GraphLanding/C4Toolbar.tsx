/**
 * C4Toolbar — toolbar with "Open in draw.io" button for C4 perspectives.
 *
 * Visible only when the perspective is c4-context, c4-container, or c4-component.
 * Fetches the C4 Mermaid diagram and opens it in draw.io.
 */
import { useCallback, useContext } from "react";
import { useAppState } from "../../state/context";
import { NotificationContext } from "../Notifications/NotificationProvider";
import { fetchC4Mermaid, fetchSnapshot } from "../../api/client";
import { handleOpenInDrawIo } from "../../utils/drawio";
import { downloadSnapshot } from "../../utils/download";
import type { C4Level } from "../../state/c4Levels";

const C4_TOOLBAR_PERSPECTIVES: C4Level[] = ["c4-context", "c4-container", "c4-component"];

export function C4Toolbar({ workspaceId }: { workspaceId: string }) {
  const { perspective } = useAppState();
  const { showNotification } = useContext(NotificationContext);

  const isC4ToolbarPerspective = C4_TOOLBAR_PERSPECTIVES.includes(
    perspective as C4Level,
  );

  const handleOpenDrawIo = useCallback(async () => {
    try {
      // Fetch C4 Mermaid diagram at the current C4 level
      const mermaidText = await fetchC4Mermaid(workspaceId, ".", perspective as C4Level);
      // Open in draw.io
      await handleOpenInDrawIo(mermaidText, { notify: showNotification });
      // Show confirmation (handleOpenInDrawIo handles this via notify callback)
    } catch (err) {
      console.error("[C4Toolbar] Failed to open in draw.io:", err);
      showNotification("Failed to open diagram in draw.io");
    }
  }, [workspaceId, perspective, showNotification]);

  // Map perspective (C4Level string) to the view_kind expected by the snapshot API
  const perspectiveToViewKind = (perspective: string): string => {
    switch (perspective) {
      case "c4-context": return "c4_context";
      case "c4-container": return "c4_container";
      case "c4-component": return "c4_component";
      default: return "c4_context";
    }
  };

  const handleDownloadSnapshot = useCallback(
    async (format: "png" | "svg") => {
      try {
        const viewKind = perspectiveToViewKind(perspective as string);
        const blob = await fetchSnapshot(workspaceId, viewKind, format, ".");
        const extension = format === "png" ? ".png" : ".svg";
        const filename = `c4-diagram${extension}`;
        await downloadSnapshot(blob, filename);
        showNotification(`Downloaded ${format.toUpperCase()} snapshot`);
      } catch (err) {
        console.error("[C4Toolbar] Failed to download snapshot:", err);
        showNotification(`Failed to download ${format.toUpperCase()} snapshot`);
      }
    },
    [workspaceId, perspective, showNotification],
  );

  if (!isC4ToolbarPerspective) {
    return null;
  }

  return (
    <div
      data-testid="c4-toolbar"
      style={{
        display: "flex",
        alignItems: "center",
        gap: 8,
        padding: "8px 16px",
        borderBottom: "1px solid var(--color-border)",
        backgroundColor: "var(--color-surface-raised)",
      }}
    >
      <button
        type="button"
        data-testid="c4-toolbar-open-drawio"
        onClick={handleOpenDrawIo}
        style={{
          display: "flex",
          alignItems: "center",
          gap: 6,
          padding: "6px 12px",
          borderRadius: 6,
          border: "1px solid var(--color-border)",
          backgroundColor: "var(--color-surface-overlay)",
          color: "var(--color-text-primary)",
          fontSize: 12,
          cursor: "pointer",
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
          <line x1="12" y1="18" x2="12" y2="12" />
          <line x1="9" y1="15" x2="15" y2="15" />
        </svg>
        Open in draw.io
      </button>
      <button
        type="button"
        data-testid="c4-toolbar-download-png"
        onClick={() => handleDownloadSnapshot("png")}
        style={{
          display: "flex",
          alignItems: "center",
          gap: 6,
          padding: "6px 12px",
          borderRadius: 6,
          border: "1px solid var(--color-border)",
          backgroundColor: "var(--color-surface-overlay)",
          color: "var(--color-text-primary)",
          fontSize: 12,
          cursor: "pointer",
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
        Download PNG
      </button>
      <button
        type="button"
        data-testid="c4-toolbar-download-svg"
        onClick={() => handleDownloadSnapshot("svg")}
        style={{
          display: "flex",
          alignItems: "center",
          gap: 6,
          padding: "6px 12px",
          borderRadius: 6,
          border: "1px solid var(--color-border)",
          backgroundColor: "var(--color-surface-overlay)",
          color: "var(--color-text-primary)",
          fontSize: 12,
          cursor: "pointer",
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
        Download SVG
      </button>
    </div>
  );
}
