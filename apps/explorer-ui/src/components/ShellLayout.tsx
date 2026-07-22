/**
 * `ShellLayout` — pure layout component.
 *
 * Receives the workspace and content slots as props.
 * Handles viewport detection and responsive grid template.
 * NO effects, NO data fetching.
 */
import { type ReactNode } from "react";

import { detectViewport, type ShellViewport } from "./viewport";
import { SkipLink } from "./SkipLink";
import { TopBar } from "./TopBar";
import { LensPanelSidebar } from "./LensPanel/LensPanelSidebar";
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
  /** Tertiary content — ContextRail or other side panel (rightmost zone). */
  tertiaryContent?: ReactNode;
  /** Shell left zone — StartRail (rendered outside the landing/workbench switch). */
  leftZone?: ReactNode;
  onSpotterOpen: () => void;
  /** e15.5 — opens the MCP Tools modal (optional) */
  onMcpToolsOpen?: () => void;
}

export function ShellLayout({
  viewport: viewportOverride,
  // eslint-disable-next-line @typescript-eslint/no-unused-vars -- intentionally unused
  workspace: _workspace,
  children,
  secondaryContent,
  tertiaryContent,
  leftZone,
  onSpotterOpen,
  onMcpToolsOpen,
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
      <TopBar onSpotterOpen={onSpotterOpen} onMcpToolsOpen={onMcpToolsOpen} />
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
              role="complementary"
              aria-label="Active panes"
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
          /* Desktop / Tablet / Ultrawide: 3-zone workbench + optional lens sidebar */
          <div className="flex h-full">
            <div
              className="grid h-full flex-1"
              style={{
                gridTemplateColumns: leftZone
                  ? tertiaryContent
                    ? "minmax(0,1.3fr) minmax(0,1fr) 20rem"
                    : "minmax(0,1.4fr) minmax(0,1fr)"
                  : tertiaryContent
                    ? "minmax(0,1.3fr) minmax(0,1fr) 20rem"
                    : "minmax(0,1.4fr) minmax(0,1fr)",
              }}
            >
              {/* Shell left zone */}
              {leftZone && (
                <div data-testid="shell-zone-left">{leftZone}</div>
              )}
              {/* Center zone — InteractiveGraph (primary) */}
              <div data-testid="shell-zone-center">{children}</div>
              {/* Right zone — PaneStackView (secondary) */}
              <div data-testid="shell-zone-right">{secondaryContent}</div>
              {tertiaryContent}
            </div>
            <LensPanelSidebar />
          </div>
        )}
      </main>
    </div>
  );
}
