/**
 * Unit tests for draw.io integration utilities.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { handleOpenInDrawIo } from "./drawio";

describe("handleOpenInDrawIo", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("writes Mermaid text to clipboard", async () => {
    const writeTextMock = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal("navigator", {
      clipboard: { writeText: writeTextMock },
    });

    const mockOpen = vi.fn();
    vi.stubGlobal("window", { open: mockOpen });

    const mermaidText = "graph TD\n  A --> B";
    await handleOpenInDrawIo(mermaidText);

    expect(writeTextMock).toHaveBeenCalledOnce();
    expect(writeTextMock).toHaveBeenCalledWith("graph TD\n  A --> B");
  });

  it("opens draw.io in a new window", async () => {
    vi.stubGlobal("navigator", {
      clipboard: { writeText: vi.fn().mockResolvedValue(undefined) },
    });

    const mockOpen = vi.fn();
    vi.stubGlobal("window", { open: mockOpen });

    await handleOpenInDrawIo("graph TD\n  A --> B");

    expect(mockOpen).toHaveBeenCalledOnce();
    expect(mockOpen).toHaveBeenCalledWith("https://app.diagrams.net/");
  });

  it("calls notify callback when provided", async () => {
    vi.stubGlobal("navigator", {
      clipboard: { writeText: vi.fn().mockResolvedValue(undefined) },
    });
    vi.stubGlobal("window", { open: vi.fn() });

    const notifyMock = vi.fn();
    await handleOpenInDrawIo("graph TD\n  A --> B", { notify: notifyMock });

    expect(notifyMock).toHaveBeenCalledOnce();
    expect(notifyMock).toHaveBeenCalledWith("Mermaid copied! In draw.io: Arrange > Insert > Mermaid");
  });

  it("does not call notify when not provided", async () => {
    vi.stubGlobal("navigator", {
      clipboard: { writeText: vi.fn().mockResolvedValue(undefined) },
    });
    vi.stubGlobal("window", { open: vi.fn() });

    // Should not throw even without notify callback
    await expect(handleOpenInDrawIo("graph TD\n  A --> B")).resolves.toBeUndefined();
  });

  it("throws error for empty mermaid text", async () => {
    vi.stubGlobal("navigator", {
      clipboard: { writeText: vi.fn().mockResolvedValue(undefined) },
    });
    vi.stubGlobal("window", { open: vi.fn() });

    await expect(handleOpenInDrawIo("")).rejects.toThrow(
      "Cannot open empty Mermaid diagram in draw.io",
    );
  });

  it("throws error for whitespace-only mermaid text", async () => {
    vi.stubGlobal("navigator", {
      clipboard: { writeText: vi.fn().mockResolvedValue(undefined) },
    });
    vi.stubGlobal("window", { open: vi.fn() });

    await expect(handleOpenInDrawIo("   ")).rejects.toThrow(
      "Cannot open empty Mermaid diagram in draw.io",
    );
  });

  it("propagates clipboard API errors", async () => {
    const writeTextMock = vi.fn().mockRejectedValue(new Error("Clipboard access denied"));
    vi.stubGlobal("navigator", {
      clipboard: { writeText: writeTextMock },
    });
    vi.stubGlobal("window", { open: vi.fn() });

    // Clipboard errors propagate - caller can catch and show user feedback
    await expect(handleOpenInDrawIo("graph TD\n  A --> B")).rejects.toThrow(
      "Clipboard access denied",
    );
  });
});
