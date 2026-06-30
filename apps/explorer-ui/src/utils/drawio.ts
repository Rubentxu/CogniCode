/**
 * Draw.io integration utilities.
 *
 * Provides a shared handler for opening Mermaid diagrams in draw.io
 * by copying the Mermaid text to clipboard and launching the editor.
 */

/**
 * Opens a Mermaid diagram in draw.io by copying the diagram text to
 * the clipboard and opening the draw.io web app.
 *
 * @param mermaidText - The Mermaid diagram text to open in draw.io
 * @returns A toast message ID for confirmation tracking
 */
export async function handleOpenInDrawIo(mermaidText: string): Promise<string> {
  await navigator.clipboard.writeText(mermaidText);
  window.open("https://app.diagrams.net/");
  // Return a toast message ID for confirmation
  return "drawio-opened";
}
