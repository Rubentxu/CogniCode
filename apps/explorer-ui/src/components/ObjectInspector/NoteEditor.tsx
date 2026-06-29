/**
 * NoteEditor — a minimal popover for editing a pane's note.
 *
 * Triggered by the 'n' keyboard shortcut on the pane header.
 * - Textarea auto-focuses on mount.
 * - Escape discards changes and closes the editor.
 * - Enter (without Shift) commits the note and closes.
 * - Clicking outside closes without saving (discard).
 */
import { useEffect, useRef, useState } from "react";
import type { Pane } from "../../state/slices/navigation/types";

type NoteEditorProps = {
  /** The pane whose note is being edited. */
  pane: Pane;
  /** Called when the user commits a note change. */
  onSave: (paneId: string, note: string) => void;
  /** Called when the user dismisses the editor (Escape or outside click). */
  onClose: () => void;
};

export function NoteEditor({ pane, onSave, onClose }: NoteEditorProps) {
  const [value, setValue] = useState(pane.note ?? "");
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Auto-focus the textarea on mount.
  useEffect(() => {
    textareaRef.current?.focus();
  }, []);

  // Trap focus inside the popover while open.
  function handleKeyDown(e: React.KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === "Escape") {
      e.stopPropagation();
      onClose();
      return;
    }
    if (e.key === "Enter" && !e.shiftKey) {
      e.stopPropagation();
      onSave(pane.id, value);
      return;
    }
    // Allow Shift+Enter to insert newline.
    e.stopPropagation();
  }

  function handleCommit() {
    onSave(pane.id, value);
  }

  return (
    <div
      data-testid="note-editor"
      className="absolute z-50 w-72 rounded-lg border p-3 shadow-lg"
      style={{
        backgroundColor: "var(--color-surface)",
        borderColor: "var(--color-border)",
      }}
    >
      <div className="mb-2 text-xs font-semibold" style={{ color: "var(--color-text-secondary)" }}>
        Note for this pane
      </div>
      <textarea
        ref={textareaRef}
        data-testid="note-editor-textarea"
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={handleKeyDown}
        rows={3}
        className="w-full resize-none rounded-md border p-2 text-xs"
        style={{
          borderColor: "var(--color-border)",
          backgroundColor: "var(--color-surface-overlay)",
          color: "var(--color-text-primary)",
          outline: "none",
        }}
        placeholder="Why did you open this pane?"
      />
      <div className="mt-2 flex items-center justify-end gap-2">
        <button
          type="button"
          data-testid="note-editor-discard"
          onClick={onClose}
          className="rounded-md px-2 py-1 text-xs"
          style={{ color: "var(--color-text-muted)" }}
        >
          Esc discard
        </button>
        <button
          type="button"
          data-testid="note-editor-save"
          onClick={handleCommit}
          className="rounded-md px-2 py-1 text-xs"
          style={{
            backgroundColor: "var(--color-primary, #3b82f6)",
            color: "white",
          }}
        >
          Enter save
        </button>
      </div>
      <div className="mt-1 text-[10px]" style={{ color: "var(--color-text-muted)" }}>
        Enter to save · Shift+Enter for newline · Esc to discard
      </div>
    </div>
  );
}
