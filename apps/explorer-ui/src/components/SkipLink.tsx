/**
 * `SkipLink` — keyboard "skip to main content" link.
 *
 * Visually hidden until focused, then snaps to the top of the
 * viewport and focuses the `<main id="app-main">` landmark.
 *
 * The `tabIndex={-1}` on the target is required for the focus to
 * land there programmatically.
 */
import { type MouseEvent } from "react";

export interface SkipLinkProps {
  /** The id of the element to focus (must be focusable). */
  targetId: string;
  /** Visible label. Defaults to "Skip to main content". */
  label?: string;
}

export function SkipLink({ targetId, label = "Skip to main content" }: SkipLinkProps) {
  const handleClick = (event: MouseEvent<HTMLAnchorElement>) => {
    event.preventDefault();
    const target = document.getElementById(targetId);
    if (target) {
      // tabIndex={-1} makes the element programmatically focusable
      // without entering the tab order — exactly what we want for a
      // skip link target.
      if (target.tabIndex < 0) target.tabIndex = -1;
      target.focus({ preventScroll: false });
    }
  };

  return (
    <a
      href={`#${targetId}`}
      onClick={handleClick}
      data-testid="skip-link"
      className="sr-only focus:not-sr-only focus:absolute focus:left-2 focus:top-2 focus:z-50 focus:rounded-md focus:px-3 focus:py-1.5 focus:text-sm focus:font-semibold"
      style={{
        // Inline fallback for focus-visible (in case the global
        // focus-visible is suppressed inside a parent class).
        outline: "2px solid var(--color-primary)",
        backgroundColor: "var(--color-surface-raised)",
        color: "var(--color-text-primary)",
      }}
    >
      {label}
    </a>
  );
}
