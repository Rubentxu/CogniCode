/**
 * RTL tests for NoteEditor.
 *
 * Verifies:
 * - Opens on mount with auto-focused textarea.
 * - Escape discards and calls onClose without saving.
 * - Enter commits and calls onSave with the note text.
 * - Shift+Enter inserts a newline (does not submit).
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { NoteEditor } from "./NoteEditor";
import type { Pane } from "../../state/slices/navigation/types";
import React from "react";

const makePane = (note?: string): Pane => ({
  id: "pane-1",
  objectId: "obj-a",
  activeViewId: null,
  activeLensId: null,
  kind: "symbol",
  activeView: null,
  scrollY: 0,
  localFilters: {},
  note,
});

describe("NoteEditor", () => {
  it("auto-focuses textarea on mount", () => {
    const onSave = vi.fn();
    const onClose = vi.fn();
    render(<NoteEditor pane={makePane()} onSave={onSave} onClose={onClose} />);
    expect(screen.getByTestId("note-editor-textarea")).toHaveFocus();
  });

  it("textarea is pre-filled with existing note", () => {
    const onSave = vi.fn();
    const onClose = vi.fn();
    render(<NoteEditor pane={makePane("existing note")} onSave={onSave} onClose={onClose} />);
    expect(screen.getByTestId("note-editor-textarea")).toHaveValue("existing note");
  });

  it("Enter submits the note and calls onSave", () => {
    const onSave = vi.fn();
    const onClose = vi.fn();
    render(<NoteEditor pane={makePane()} onSave={onSave} onClose={onClose} />);
    const textarea = screen.getByTestId("note-editor-textarea");
    fireEvent.change(textarea, { target: { value: "my note" } });
    fireEvent.keyDown(textarea, { key: "Enter", shiftKey: false });
    expect(onSave).toHaveBeenCalledWith("pane-1", "my note");
    expect(onClose).not.toHaveBeenCalled();
  });

  it("Shift+Enter inserts newline and does not submit", () => {
    const onSave = vi.fn();
    const onClose = vi.fn();
    render(<NoteEditor pane={makePane()} onSave={onSave} onClose={onClose} />);
    const textarea = screen.getByTestId("note-editor-textarea");
    fireEvent.change(textarea, { target: { value: "line1\nline2" } });
    fireEvent.keyDown(textarea, { key: "Enter", shiftKey: true });
    expect(onSave).not.toHaveBeenCalled();
  });

  it("Escape calls onClose without saving", () => {
    const onSave = vi.fn();
    const onClose = vi.fn();
    render(<NoteEditor pane={makePane()} onSave={onSave} onClose={onClose} />);
    const textarea = screen.getByTestId("note-editor-textarea");
    fireEvent.change(textarea, { target: { value: "unsaved text" } });
    fireEvent.keyDown(textarea, { key: "Escape" });
    expect(onSave).not.toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
  });

  it("Save button calls onSave with current value", () => {
    const onSave = vi.fn();
    const onClose = vi.fn();
    render(<NoteEditor pane={makePane()} onSave={onSave} onClose={onClose} />);
    const textarea = screen.getByTestId("note-editor-textarea");
    fireEvent.change(textarea, { target: { value: "button save" } });
    fireEvent.click(screen.getByTestId("note-editor-save"));
    expect(onSave).toHaveBeenCalledWith("pane-1", "button save");
  });

  it("Discard button calls onClose without saving", () => {
    const onSave = vi.fn();
    const onClose = vi.fn();
    render(<NoteEditor pane={makePane()} onSave={onSave} onClose={onClose} />);
    fireEvent.change(screen.getByTestId("note-editor-textarea"), {
      target: { value: "discarded" },
    });
    fireEvent.click(screen.getByTestId("note-editor-discard"));
    expect(onSave).not.toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
  });
});
