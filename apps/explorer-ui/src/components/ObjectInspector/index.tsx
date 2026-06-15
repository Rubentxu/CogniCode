/**
 * `ObjectInspector` — the centre panel of the Explorer.
 *
 * Thin wrapper that reads `activeObjectId` from the app state and
 * renders a single `PaneInspector`. In pane-stack mode the
 * `PaneStackView` renders multiple `PaneInspector`s directly
 * (without going through this wrapper).
 *
 * See `PaneInspector` for the core rendering logic.
 */
import { useApp } from "../../state/context";
import { PaneInspector } from "./PaneInspector";

export { ViewTabs } from "./ViewTabs";
export type { ViewTabsProps } from "./ViewTabs";
export { ViewBlock, Blocks } from "./ViewBlock";
export type { ViewBlockProps, BlocksProps } from "./ViewBlock";
export { SuggestionStrip } from "./SuggestionStrip";
export type { SuggestionStripProps } from "./SuggestionStrip";

export function ObjectInspector() {
  const { state } = useApp();
  const { activeObjectId, activeViewId, activeLensId, activeView } = state;

  if (!activeObjectId) {
    return (
      <div
        data-testid="object-inspector-empty"
        className="flex h-full items-center justify-center p-6 text-center text-sm"
        style={{ color: "var(--color-text-secondary)" }}
      >
        <div>
          <p
            className="font-semibold"
            style={{ color: "var(--color-text-primary)" }}
          >
            No object selected
          </p>
          <p className="mt-1 text-xs">
            Drill into the Miller Columns or open the Spotter.
          </p>
        </div>
      </div>
    );
  }

  return (
    <PaneInspector
      objectId={activeObjectId}
      viewId={activeViewId}
      lensId={activeLensId}
      activeView={activeView}
    />
  );
}
