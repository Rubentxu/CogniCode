/**
 * Download utilities for exporting snapshots and other binary content.
 *
 * Provides a safe `downloadSnapshot` helper that creates an object URL,
 * triggers a browser download, and revokes the URL after a tick to avoid
 * race conditions with the download initiating.
 */

/**
 * Trigger a browser download from a Blob.
 *
 * The URL is revoked after 100ms to ensure the download has started before
 * cleanup, avoiding the race between `URL.revokeObjectURL` and the async download.
 */
export async function downloadSnapshot(data: Blob, filename: string): Promise<void> {
  const url = URL.createObjectURL(data);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  // Revoke after a tick to ensure download starts
  setTimeout(() => URL.revokeObjectURL(url), 100);
}
