/**
 * `Item` — a single row inside a Miller Column.
 *
 * Pure presentational. Receives its a11y + click props from the
 * `Column` parent (so the roving tabindex lives in one place).
 *
 * Three visual modes:
 * - `kind` icon + name + optional `expandable` chevron
 * - Highlighted when `aria-selected` (i.e. the active item)
 */
import { type ReactNode } from "react";

export interface ItemProps {
  /** Display label. */
  name: string;
  /** Object kind ("symbol" | "file" | "scope" | etc.) — drives the icon. */
  kind: string;
  /** Whether the user can expand this item (ArrowRight) into a child column. */
  expandable: boolean;
  /** Active (focused) state from the roving tabindex. */
  active: boolean;
  /** A11y + click wiring from `useRovingFocus.getItemProps()`. */
  itemProps: {
    id: string;
    role: "option";
    tabIndex: number;
    "aria-selected"?: boolean;
    onClick: () => void;
  };
  /** Optional secondary line (file:line) — rendered muted. */
  subtitle?: string;
}

/**
 * Tiny kind-to-glyph map. We use a single character instead of an
 * icon font to keep the bundle small — Miller Columns can render
 * thousands of items and the icon set is intentionally narrow.
 */
function kindGlyph(kind: string): string {
  switch (kind) {
    case "symbol":
      return "ƒ";
    case "file":
      return "□";
    case "scope":
      return "▤";
    case "module":
      return "▦";
    case "workspace":
      return "◉";
    case "evidence":
      return "▣";
    case "decision_artifact":
      return "✎";
    case "quality_issue":
      return "!";
    case "rule":
      return "§";
    default:
      return "•";
  }
}

export function Item({
  name,
  kind,
  expandable,
  active,
  itemProps,
  subtitle,
}: ItemProps) {
  return (
    <li
      {...itemProps}
      data-testid={`miller-item-${kind}`}
      data-active={active ? "true" : "false"}
      className="group flex cursor-pointer items-center gap-2 rounded-sm px-2 py-1 text-sm transition-colors"
      style={{
        backgroundColor: active ? "var(--color-surface-overlay)" : "transparent",
        color: "var(--color-text-primary)",
      }}
      onMouseEnter={(event) => {
        if (!active) {
          event.currentTarget.style.backgroundColor = "var(--color-surface-raised)";
        }
      }}
      onMouseLeave={(event) => {
        if (!active) {
          event.currentTarget.style.backgroundColor = "transparent";
        }
      }}
    >
      <span
        aria-hidden="true"
        className="inline-flex h-4 w-4 flex-none items-center justify-center font-mono text-xs"
        style={{ color: "var(--color-text-muted)" }}
      >
        {kindGlyph(kind)}
      </span>
      <span className="min-w-0 flex-1 truncate" title={name}>
        {name}
      </span>
      {subtitle && (
        <span
          className="ml-auto truncate text-xs"
          style={{ color: "var(--color-text-muted)" }}
          title={subtitle}
        >
          {subtitle}
        </span>
      )}
      {expandable && (
        <span
          aria-hidden="true"
          className="ml-1 flex-none font-mono text-xs"
          style={{ color: "var(--color-text-muted)" }}
        >
          ›
        </span>
      )}
    </li>
  );
}

/**
 * Helper for tests and custom renderers that need to render an
 * arbitrary node inside the column's list (e.g. the loading skeleton
 * and empty state). Re-exported so consumers do not have to import
 * ReactNode just to type a slot.
 */
export type ItemSlot = ReactNode;
