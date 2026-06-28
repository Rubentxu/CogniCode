/**
 * `IntentFooter` — chip strip below Spotter results.
 *
 * Renders one chip per applicable view of the currently-highlighted
 * result. Plus two disabled placeholder chips for forward-compat:
 *   - "Open as C4 context"   (E19 — C4 executors not yet wired)
 *   - "Add to investigation"  (E21 — Investigation entity not yet wired)
 *
 * Keyboard shortcuts: Cmd+1..N pick the nth enabled chip. The
 * `spotter-intent-{viewId}` testid enables E2E selection.
 */
import type { SpotterResult } from "../../api/types";

export interface IntentChip {
  viewId: string;
  label: string;
  shortcut?: string; // e.g., "Cmd+1"
  disabled?: boolean;
  comingSoon?: boolean;
  title?: string;
}

export interface IntentFooterProps {
  /** Currently highlighted result, or null if no selection. */
  result: SpotterResult | null;
  /** Called when user picks a chip (click or keyboard). */
  onPick: (viewId: string) => void;
  /** Total chips (for "Cmd+N" hint). Used to show shortcut labels. */
  index?: number;
}

/**
 * Map a SpotterResult's available_views into chips.
 * Deduplicates by viewId. Adds the two forward-compat placeholders.
 */
export function chipsFromResult(
  result: SpotterResult | null
): IntentChip[] {
  if (!result) return [];
  const seen = new Set<string>();
  const chips: IntentChip[] = [];
  const views = result.object.available_views ?? [];
  for (const v of views) {
    if (!v?.id || seen.has(v.id)) continue;
    seen.add(v.id);
    chips.push({ viewId: v.id, label: v.title ?? v.id });
  }
  // Forward-compat placeholders (E19, E21)
  chips.push({
    viewId: "c4-context",
    label: "Open as C4",
    disabled: true,
    comingSoon: true,
    title: "Coming in E19",
  });
  chips.push({
    viewId: "add-to-investigation",
    label: "Add to Investigation",
    disabled: true,
    comingSoon: true,
    title: "Coming in E21",
  });
  return chips;
}

export function IntentFooter({ result, onPick, index = 0 }: IntentFooterProps) {
  const chips = chipsFromResult(result);

  if (!result || chips.length === 0) {
    return (
      <div
        data-testid="spotter-intent-footer"
        className="flex items-center gap-1 border-t px-2 py-1.5 text-xs"
        style={{
          borderColor: "var(--color-border)",
          color: "var(--color-text-muted)",
        }}
      >
        Pick a result to see views
      </div>
    );
  }

  return (
    <div
      data-testid="spotter-intent-footer"
      role="toolbar"
      aria-label="Open as view"
      className="flex flex-wrap items-center gap-1 border-t px-2 py-1.5"
      style={{ borderColor: "var(--color-border)" }}
    >
      {chips.map((chip, i) => {
        const shortcut = chip.disabled ? undefined : `Cmd+${i + 1}`;
        return (
          <button
            key={chip.viewId}
            type="button"
            disabled={chip.disabled}
            aria-disabled={chip.disabled ? "true" : undefined}
            title={chip.title ?? (shortcut ? `${chip.label} (${shortcut})` : chip.label)}
            data-testid={`spotter-intent-${chip.viewId}`}
            onClick={() => {
              if (!chip.disabled) onPick(chip.viewId);
            }}
            className="rounded-full px-2 py-0.5 text-xs font-medium"
            style={{
              backgroundColor:
                chip.disabled
                  ? "var(--color-surface-overlay)"
                  : "var(--color-surface-overlay)",
              color: chip.disabled
                ? "var(--color-text-muted)"
                : "var(--color-text-primary)",
              opacity: chip.disabled ? 0.5 : 1,
              cursor: chip.disabled ? "not-allowed" : "pointer",
            }}
          >
            {chip.label}
            {!chip.disabled && (
              <span
                aria-hidden="true"
                className="ml-1 font-mono text-[10px]"
                style={{ color: "var(--color-text-muted)" }}
              >
                {shortcut}
              </span>
            )}
            {chip.comingSoon && (
              <span
                aria-hidden="true"
                className="ml-1 text-[10px]"
                style={{ color: "var(--color-text-muted)" }}
              >
                soon
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
}
