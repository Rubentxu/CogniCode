/**
 * Draw.io integration utilities.
 *
 * Provides a shared handler for opening Mermaid diagrams in draw.io
 * by copying the Mermaid text to clipboard and launching the editor.
 */

export interface DrawIoOptions {
  /** Optional notifier function for user feedback */
  notify?: (message: string) => void;
}

/**
 * Opens a Mermaid diagram in draw.io by copying the diagram text to
 * the clipboard and opening the draw.io web app.
 *
 * @param mermaidText - The Mermaid diagram text to open in draw.io
 * @param options - Optional configuration including notification callback
 * @throws Error if mermaidText is empty or whitespace-only
 */
export async function handleOpenInDrawIo(
  mermaidText: string,
  options?: DrawIoOptions,
): Promise<void> {
  const trimmed = mermaidText.trim();
  if (!trimmed) {
    throw new Error("Cannot open empty Mermaid diagram in draw.io");
  }
  await navigator.clipboard.writeText(trimmed);
  window.open("https://app.diagrams.net/");
  options?.notify?.("Mermaid copied! In draw.io: Arrange > Insert > Mermaid");
}
