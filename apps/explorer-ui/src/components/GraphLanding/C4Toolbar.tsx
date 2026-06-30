/**
 * C4Toolbar — toolbar with "Open in draw.io" button for C4 perspectives.
 *
 * Visible only when the perspective is c4-context, c4-container, or c4-component.
 * Fetches the C4 Mermaid diagram and opens it in draw.io.
 */
import { useCallback, useContext } from "react";
import { useAppState } from "../../state/context";
import { NotificationContext } from "../Notifications/NotificationProvider";
import { fetchC4Mermaid } from "../../api/client";
import { handleOpenInDrawIo } from "../../utils/drawio";
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
    </div>
  );
}
