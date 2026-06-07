/**
 * `Column` — a single Miller Column.
 *
 * Renders a header (label + count), the list of items via `useRovingFocus`,
 * the live-region announcer, and the per-column ErrorBoundary + LoadingTier
 * that the design calls for.
 *
 * The column does NOT decide what to render inside it — it gets a list
 * of items from the parent (`MillerColumns` maps a column's `object_id`
 * to its children via SWR). The parent's `useRovingFocus` lifecycle
 * owns the keyboard contract; the column just exposes the props.
 */
/* eslint-disable react-hooks/refs --
   The `roving` prop carries a live-region ref by design — the column
   attaches it to a <p aria-live> in JSX. The linter flags any read of
   the API object as a ref access; this is intentional. */
import { useEffect, useMemo } from "react";

import { ErrorBoundary } from "../ErrorBoundary";
import { LoadingTier } from "../LoadingTier";
import type { RovingFocusApi } from "../../hooks/useRovingFocus";
import { Item } from "./Item";

export interface MillerColumnItem {
  /** Stable id used as the SWR key. */
  id: string;
  /** Display label. */
  name: string;
  /** Object kind (drives the glyph and determines expandability). */
  kind: string;
  /** Whether ArrowRight opens a child column. */
  expandable: boolean;
  /** Optional secondary line. */
  subtitle?: string;
}

export interface ColumnProps {
  /** Position in the Miller Columns row (0-based). */
  index: number;
  /** Header text. Typically the active object label. */
  label: string;
  /** Items to render. `undefined` while loading; `null` for empty. */
  items: MillerColumnItem[] | null | undefined;
  /** Whether the SWR fetch is still in flight. */
  isLoading: boolean;
  /** Error from the SWR fetch (per-column). */
  error?: Error | null;
  /** True if this column is the last one in the row. */
  isActive: boolean;
  /** Optional SWR revalidation indicator. */
  isValidating?: boolean;
  /** Roving focus API from the parent (so all columns share state). */
  roving: RovingFocusApi;
  /** True when the column is mounted but the user has not yet focused it. */
  autoFocus?: boolean;
  /**
   * Called by the column when its on-screen item count changes —
   * the parent uses this to clamp the focus index in the shared
   * roving-focus state.
   */
  onItemCountChange?: (count: number) => void;
}

export function Column({
  index,
  label,
  items,
  isLoading,
  error = null,
  isActive,
  isValidating = false,
  roving,
  autoFocus = false,
  onItemCountChange,
}: ColumnProps) {
  const itemCount = items?.length ?? 0;
  const containerLabel = useMemo(
    () => `${label} (${itemCount} ${itemCount === 1 ? "item" : "items"})`,
    [label, itemCount],
  );

  // Notify the parent when the item count changes so it can keep the
  // shared roving focus in sync.
  useEffect(() => {
    onItemCountChange?.(itemCount);
  }, [itemCount, onItemCountChange]);

  // Auto-focus the column on mount when requested. We only set
  // focus when the user has not yet interacted with another column
  // — this keeps the initial Tab journey predictable.
  const activeIndex = roving.activeIndex;
  useEffect(() => {
    if (!autoFocus) return;
    if (typeof document === "undefined") return;
    const id = `miller-column-${index}-item-${activeIndex}`;
    const el = document.getElementById(id);
    el?.focus({ preventScroll: false });
  }, [autoFocus, activeIndex, index]);

  // Snapshot the container props synchronously — `roving` carries
  // refs and we only want the onKeyDown/role/tabIndex/aria-label
  // tuple for the JSX below.
  const containerProps = roving.getContainerProps();

  return (
    <ErrorBoundary label={`MillerColumn:${index}`}>
      <section
        data-testid={`miller-column-${index}`}
        data-active={isActive ? "true" : "false"}
        aria-labelledby={`miller-column-${index}-label`}
        className="flex h-full min-w-0 flex-col"
        style={{
          minWidth: "var(--spacing-column-min-width)",
          maxWidth: "var(--spacing-column-max-width)",
          backgroundColor: "var(--color-surface-raised)",
          borderRight: "1px solid var(--color-border)",
        }}
      >
        <header
          className="flex items-center justify-between gap-2 px-3 py-2"
          style={{ borderBottom: "1px solid var(--color-border)" }}
        >
          <h3
            id={`miller-column-${index}-label`}
            className="truncate text-xs font-semibold uppercase tracking-wide"
            style={{ color: "var(--color-text-secondary)" }}
            title={label}
          >
            {label}
          </h3>
          <span
            aria-hidden="true"
            className="rounded-full px-1.5 py-0.5 text-xs"
            style={{
              backgroundColor: "var(--color-surface-overlay)",
              color: "var(--color-text-muted)",
            }}
          >
            {itemCount}
          </span>
        </header>
        <LoadingTier
          data={items}
          isLoading={isLoading}
          isValidating={isValidating}
          error={error}
          label={`Miller column ${index + 1}`}
          emptyMessage="No items to show"
        >
          <ul
            {...containerProps}
            aria-label={containerLabel}
            className="flex flex-1 flex-col gap-0.5 overflow-y-auto p-2"
            style={{ outline: "none" }}
          >
            {items?.map((item, idx) => {
              const itemProps = roving.getItemProps(idx, index);
              return (
                <Item
                  key={item.id}
                  name={item.name}
                  kind={item.kind}
                  expandable={item.expandable}
                  active={idx === roving.activeIndex}
                  itemProps={itemProps}
                  subtitle={item.subtitle}
                />
              );
            })}
          </ul>
        </LoadingTier>
        <p
          ref={roving.liveRegionRef}
          aria-live="polite"
          aria-atomic="true"
          className="sr-only"
        />
      </section>
    </ErrorBoundary>
  );
}
