/**
 * `ShellLayout` — pure layout component.
 *
 * Receives the workspace and content slots as props.
 * Handles viewport detection and responsive grid template.
 * NO effects, NO data fetching.
 */
import { type ReactNode } from "react";

import { detectViewport, type ShellViewport } from "./viewport";
import { HealthProbe } from "./HealthProbe";
import { SkipLink } from "./SkipLink";
import { PerspectiveToggle } from "./PerspectiveToggle";
import { ScanBar } from "./ScanBar";
import { ShareExplorationButton } from "./ShareExplorationButton";
import type { WorkspaceSummary } from "../api/types";

export interface ShellLayoutProps {
  /**
   * Override the viewport. Used by tests + Playwright to assert the
   * responsive behaviour without resizing the window.
   */
  viewport?: ShellViewport;
  workspace: WorkspaceSummary | null;
  /**
   * Primary content — InteractiveGraphPanel or GraphLanding.
   * In small viewport: full-width graph.
   * In desktop viewport: left zone of the 2-zone grid.
   */
  children: ReactNode;
  /**
   * Secondary content — PaneStackView.
   * In small viewport: rendered inside the bottom sheet overlay.
   * In desktop viewport: right zone of the 2-zone grid.
   */
  secondaryContent: ReactNode;
  onSpotterOpen: () => void;
}

export function ShellLayout({
  viewport: viewportOverride,
  // eslint-disable-next-line @typescript-eslint/no-unused-vars -- intentionally unused
  workspace: _workspace,
  children,
  secondaryContent,
  onSpotterOpen,
}: ShellLayoutProps) {
  const activeViewport: ShellViewport = viewportOverride ?? detectViewport(
    typeof window !== "undefined" ? window.innerWidth : 1200,
  );
  const isSmall = activeViewport === "small";

  return (
    <div
      data-testid="shell"
      data-viewport={activeViewport}
      className="flex h-full w-full flex-col"
      style={{ backgroundColor: "var(--color-surface)" }}
    >
      <SkipLink targetId="app-main" />
      {/* Top bar */}
      <header
        className="flex items-center justify-between gap-3 px-4 py-2"
        style={{
          backgroundColor: "var(--color-surface-raised)",
          borderBottom: "1px solid var(--color-border)",
        }}
      >
        <div className="flex items-center gap-2">
          <h1
            className="text-sm font-semibold"
            style={{ color: "var(--color-text-primary)" }}
          >
            CogniCode Explorer
          </h1>
          <HealthProbe showFullScreenOnError={false} />
          <ScanBar />
          <PerspectiveToggle />
        </div>
        <div className="flex items-center gap-2">
          <ShareExplorationButton />
          <button
            type="button"
            onClick={onSpotterOpen}
            aria-label="Open Spotter search"
            data-testid="spotter-trigger"
            className="flex items-center gap-2 rounded-md px-2 py-1 text-xs"
            style={{
              backgroundColor: "var(--color-surface-overlay)",
              color: "var(--color-text-secondary)",
              border: "1px solid var(--color-border)",
            }}
          >
            <span aria-hidden="true">⌕</span>
            <span>Search</span>
            <span
              aria-hidden="true"
              className="rounded px-1 font-mono text-xs"
              style={{
                backgroundColor: "var(--color-surface)",
                color: "var(--color-text-muted)",
              }}
            >
              ⌘K
            </span>
          </button>
        </div>
      </header>
      <main
        id="app-main"
        tabIndex={-1}
        className="flex-1 overflow-hidden"
        aria-label="Explorer panels"
      >
        {isSmall ? (
          <div className="relative grid h-full" style={{ gridTemplateColumns: "1fr" }}>
            {/* Graph — full width on small viewport */}
            {children}
            {/* Bottom sheet — PaneStackView slides up from bottom */}
            <div
              data-testid="bottom-sheet"
              className="absolute left-0 right-0 top-1/2 z-20"
              style={{
                bottom: 0,
                height: "60vh",
                backgroundColor: "var(--color-surface)",
                borderTop: "1px solid var(--color-border)",
                boxShadow: "0 -8px 24px rgba(0,0,0,0.35)",
              }}
            >
              {secondaryContent}
            </div>
          </div>
        ) : (
          /* Desktop / Tablet / Ultrawide: 2-zone grid */
          <div
            className="grid h-full"
            style={{ gridTemplateColumns: "minmax(0,1.4fr) minmax(0,1fr)" }}
          >
            {/* Left — InteractiveGraph (primary) */}
            {children}
            {/* Right — PaneStackView (secondary) */}
            {secondaryContent}
          </div>
        )}
      </main>
    </div>
  );
}
