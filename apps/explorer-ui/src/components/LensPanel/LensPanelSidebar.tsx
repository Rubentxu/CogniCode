/**
 * `LensPanelSidebar` — collapsible right sidebar wrapping the existing
 * LensPanel. Adds:
 * - Toggle button in shell header
 * - localStorage persistence
 * - Viewport-aware visibility (desktop vs mobile)
 */

import { useEffect, useState } from "react";

import { useAppSelector } from "../../state/context";
import { detectViewport, type ShellViewport } from "../viewport";
import { LensPanel } from "../LensPanel";

export function LensPanelSidebar(): JSX.Element | null {
  const open = useAppSelector((s) => s.lensSidebar.open);

  // Determine viewport.
  const [viewport, setViewport] = useState<ShellViewport>(() =>
    typeof window === "undefined"
      ? "desktop"
      : detectViewport(window.innerWidth),
  );
  useEffect(() => {
    if (typeof window === "undefined") return;
    const onResize = () => setViewport(detectViewport(window.innerWidth));
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  if (!open) return null;

  // The LensPanel itself receives workspaceId + optional object context.
  // On mobile/tablet the sidebar is full-width overlay (not yet wired;
  // Phase 2 will add the overlay backdrop).
  return (
    <aside
      data-testid="lens-panel-sidebar"
      role="complementary"
      aria-label="Analysis lenses"
      className="flex h-full flex-col border-l"
      style={{
        width: viewport === "small" ? "100%" : "320px",
        flexShrink: 0,
        backgroundColor: "var(--color-surface)",
        borderColor: "var(--color-border)",
      }}
    >
      <LensPanel viewport={viewport} />
    </aside>
  );
}
