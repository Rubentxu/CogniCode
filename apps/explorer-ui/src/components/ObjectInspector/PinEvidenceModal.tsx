/**
 * `PinEvidenceModal` — ADR-005 E21-2.
 *
 * Allows the user to pin an evidence note to an investigation.
 * User selects which investigation from the dropdown of active/draft investigations.
 * Displayed as an overlay modal triggered by the Pin button in the
 * PaneInspector header.
 */
import { useState } from "react";
import { useInvestigations, pinEvidence } from "../../hooks/useInvestigations";

interface PinEvidenceModalProps {
  /** The object being pinned as evidence. */
  objectId: string;
  /** The view id when the evidence was captured (optional). */
  viewId: string | null;
  /** Workspace ID for fetching investigations. */
  workspaceId: string | null;
  /** Called when the modal should close (success, cancel, or error). */
  onClose: () => void;
}

export function PinEvidenceModal({
  objectId,
  viewId,
  workspaceId,
  onClose,
}: PinEvidenceModalProps) {
  const { data: investigations, isLoading: isLoadingInvestigations } = useInvestigations(workspaceId ?? null);
  const [note, setNote] = useState("");
  const [selectedInvestigationId, setSelectedInvestigationId] = useState<string>("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);

  // Filter only active or draft investigations
  const activeInvestigations =
    investigations?.filter((inv) => inv.status === "active" || inv.status === "draft") ?? [];

  // Pre-select first investigation if available (only once on mount)
  if (activeInvestigations.length > 0 && !selectedInvestigationId) {
    setSelectedInvestigationId(activeInvestigations[0].id);
  }

  async function handlePin() {
    if (!note.trim() || !selectedInvestigationId) return;
    setLoading(true);
    setError(null);
    try {
      await pinEvidence(selectedInvestigationId, {
        object_id: objectId,
        view_id: viewId ?? undefined,
        note: note.trim(),
      });
      setSuccess(true);
      // Auto-close after brief success display.
      setTimeout(onClose, 800);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      onClose();
    }
  }

  const canPin = note.trim() && selectedInvestigationId && !loading;

  return (
    <div
      data-testid="pin-evidence-modal"
      role="dialog"
      aria-modal="true"
      aria-label="Pin evidence"
      className="absolute inset-0 z-50 flex items-center justify-center"
      style={{ backgroundColor: "rgba(0,0,0,0.45)" }}
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
      onKeyDown={handleKeyDown}
    >
      <div
        data-testid="pin-evidence-modal-panel"
        className="flex flex-col rounded-lg p-6 shadow-xl"
        style={{
          width: "440px",
          maxWidth: "90vw",
          backgroundColor: "var(--color-surface-raised)",
          border: "1px solid var(--color-border)",
        }}
      >
        {/* Header */}
        <div className="mb-4 flex items-center justify-between">
          <h2
            className="text-base font-semibold"
            style={{ color: "var(--color-text-primary)" }}
          >
            Pin Evidence
          </h2>
          <button
            type="button"
            onClick={onClose}
            data-testid="pin-evidence-modal-close"
            aria-label="Close"
            className="text-sm"
            style={{ color: "var(--color-text-muted)" }}
          >
            ✕
          </button>
        </div>

        {/* Object being pinned (read-only context) */}
        <div className="mb-3 rounded p-2" style={{ backgroundColor: "var(--color-surface-overlay)" }}>
          <p className="text-xs font-medium" style={{ color: "var(--color-text-muted)" }}>
            Object
          </p>
          <p
            className="truncate font-mono text-sm"
            style={{ color: "var(--color-text-primary)" }}
            title={objectId}
          >
            {objectId}
          </p>
        </div>

        {/* Investigation selector */}
        {isLoadingInvestigations ? (
          <div className="mb-4">
            <p className="text-xs" style={{ color: "var(--color-text-muted)" }}>
              Loading investigations…
            </p>
          </div>
        ) : activeInvestigations.length === 0 ? (
          <div className="mb-4">
            <p className="text-xs" style={{ color: "var(--color-text-error)" }}>
              No active or draft investigations found. Create one from the Landing Workbench first.
            </p>
          </div>
        ) : (
          <div className="mb-4">
            <label
              htmlFor="pin-evidence-investigation"
              className="mb-1 block text-xs font-medium"
              style={{ color: "var(--color-text-secondary)" }}
            >
              Investigation <span style={{ color: "var(--color-text-error)" }}>*</span>
            </label>
            <select
              id="pin-evidence-investigation"
              data-testid="pin-evidence-investigation"
              value={selectedInvestigationId}
              onChange={(e) => setSelectedInvestigationId(e.target.value)}
              className="w-full rounded px-3 py-2 text-sm"
              style={{
                backgroundColor: "var(--color-surface-overlay)",
                color: "var(--color-text-primary)",
                border: "1px solid var(--color-border)",
              }}
            >
              <option value="">— Select investigation —</option>
              {activeInvestigations.map((inv) => (
                <option key={inv.id} value={inv.id}>
                  {inv.title} ({inv.status})
                </option>
              ))}
            </select>
          </div>
        )}

        {/* Note textarea */}
        <div className="mb-4">
          <label
            htmlFor="pin-evidence-note"
            className="mb-1 block text-xs font-medium"
            style={{ color: "var(--color-text-secondary)" }}
          >
            Why is this relevant?{" "}
            <span style={{ color: "var(--color-text-error)" }}>*</span>
          </label>
          <textarea
            id="pin-evidence-note"
            data-testid="pin-evidence-note"
            value={note}
            onChange={(e) => setNote(e.target.value)}
            placeholder="Explain why this symbol/route/decision matters for the investigation…"
            rows={4}
            className="w-full rounded px-3 py-2 text-sm"
            style={{
              backgroundColor: "var(--color-surface-overlay)",
              color: "var(--color-text-primary)",
              border: "1px solid var(--color-border)",
              resize: "vertical",
            }}
          />
        </div>

        {/* Error */}
        {error && (
          <div
            data-testid="pin-evidence-error"
            className="mb-4 rounded p-3 text-sm"
            style={{
              backgroundColor:
                "color-mix(in srgb, var(--color-text-error) 10%, transparent)",
              color: "var(--color-text-error)",
              border:
                "1px solid color-mix(in srgb, var(--color-text-error) 30%, transparent)",
            }}
          >
            {error}
          </div>
        )}

        {/* Success */}
        {success && (
          <div
            data-testid="pin-evidence-success"
            className="mb-4 rounded p-3 text-sm"
            style={{
              backgroundColor:
                "color-mix(in srgb, var(--color-accent-success, #22c55e) 10%, transparent)",
              color: "var(--color-accent-success, #22c55e)",
              border:
                "1px solid color-mix(in srgb, var(--color-accent-success, #22c55e) 30%, transparent)",
            }}
          >
            Evidence pinned successfully.
          </div>
        )}

        {/* Actions */}
        <div className="flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            data-testid="pin-evidence-cancel"
            disabled={loading}
            className="rounded px-4 py-2 text-sm"
            style={{
              backgroundColor: "var(--color-surface-overlay)",
              color: "var(--color-text-secondary)",
              border: "1px solid var(--color-border)",
              cursor: loading ? "not-allowed" : "pointer",
            }}
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={handlePin}
            disabled={!canPin}
            data-testid="pin-evidence-submit"
            className="rounded px-4 py-2 text-sm font-medium"
            style={{
              backgroundColor: canPin ? "var(--color-accent)" : "var(--color-surface-overlay)",
              color: canPin ? "white" : "var(--color-text-muted)",
              cursor: canPin ? "pointer" : "not-allowed",
              opacity: loading ? 0.7 : 1,
            }}
          >
            {loading ? "Pinning…" : "Pin Evidence"}
          </button>
        </div>
      </div>
    </div>
  );
}