/**
 * `TopBar` — extracted from ShellLayout.
 *
 * Contains: heading, health-chip, spotter-trigger, MCP tools,
 * perspective toggle, share, view-spec wizard, and lens controls.
 */
import { HealthProbe } from "./HealthProbe";
import { PerspectiveToggle } from "./PerspectiveToggle";
import { ScanBar } from "./ScanBar";
import { ShareExplorationButton } from "./ShareExplorationButton";
import { LensSidebarToggle } from "./LensSidebarToggle";
import { ViewSpecWizardTrigger } from "./ViewSpecWizardTrigger";

export interface TopBarProps {
  onSpotterOpen: () => void;
  /** e15.5 — opens the MCP Tools modal (optional) */
  onMcpToolsOpen?: () => void;
}

export function TopBar({ onSpotterOpen, onMcpToolsOpen }: TopBarProps) {
  return (
    <header
      data-testid="topbar"
      className="flex items-center justify-between gap-4 px-4 py-2.5"
      style={{
        backgroundColor: "var(--color-surface-raised)",
        borderBottom: "1px solid var(--color-border)",
      }}
    >
      <div className="flex min-w-0 items-center gap-3">
        <div className="min-w-0">
          <h1
            data-testid="topbar-brand"
            className="truncate text-sm font-semibold"
            style={{ color: "var(--color-text-primary)" }}
          >
            CogniCode Explorer
          </h1>
          <p
            data-testid="topbar-tagline"
            className="truncate text-[11px]"
            style={{ color: "var(--color-text-muted)" }}
          >
            Moldable exploration workbench
          </p>
        </div>
        <div data-testid="topbar-status" className="flex items-center gap-3">
          <HealthProbe showFullScreenOnError={false} />
          <ScanBar />
          <PerspectiveToggle />
        </div>
      </div>
      <div data-testid="topbar-actions" className="flex items-center gap-2">
        <button
          type="button"
          onClick={onSpotterOpen}
          aria-label="Open Spotter search"
          data-testid="spotter-trigger"
          className="flex items-center gap-2 rounded-lg px-3 py-1.5 text-xs font-medium"
          style={{
            backgroundColor: "color-mix(in srgb, var(--color-primary) 16%, var(--color-surface-overlay))",
            color: "var(--color-text-primary)",
            border: "1px solid color-mix(in srgb, var(--color-primary) 45%, var(--color-border))",
          }}
        >
          <span aria-hidden="true">⌕</span>
          <span>Spotter</span>
          <span
            aria-hidden="true"
            className="rounded px-1 font-mono text-[11px]"
            style={{
              backgroundColor: "var(--color-surface)",
              color: "var(--color-text-muted)",
            }}
          >
            ⌘K
          </span>
        </button>
        <ShareExplorationButton />
        <ViewSpecWizardTrigger />
        <LensSidebarToggle />
        {onMcpToolsOpen && (
          <button
            type="button"
            onClick={onMcpToolsOpen}
            aria-label="Open MCP tools"
            data-testid="mcp-tools-trigger"
            className="flex items-center gap-1.5 rounded-md px-2 py-1 text-xs"
            style={{
              backgroundColor: "var(--color-surface-overlay)",
              color: "var(--color-text-secondary)",
              border: "1px solid var(--color-border)",
            }}
          >
            <span aria-hidden="true">⚙</span>
            <span>Tools</span>
          </button>
        )}
      </div>
    </header>
  );
}
