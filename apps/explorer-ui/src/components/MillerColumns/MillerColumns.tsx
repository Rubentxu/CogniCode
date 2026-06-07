/**
 * `MillerColumns` — the leftmost panel of the Explorer.
 *
 * Renders one column per entry in `state.columns` (a path of object
 * ids). Each column fetches its children via the appropriate SWR
 * hook based on the parent object's `kind`.
 *
 * Keyboard wiring (driven by the shared `useRovingFocus` for the
 * ACTIVE column only — inactive columns are inert):
 * - ArrowDown / ArrowUp: move focus within the active column.
 * - ArrowRight on a focused item: dispatches PUSH_COLUMN (or
 *   SELECT_OBJECT for the leaf).
 * - ArrowLeft: dispatches POP_COLUMN.
 * - Enter: dispatches SELECT_OBJECT (triggers the Object Inspector).
 * - Escape: pops the current column.
 * - Home / End: jumps to first / last item.
 * - PageUp / PageDown: jumps by 10 items.
 *
 * Tab is NOT intercepted — it moves focus to the next focusable
 * element in document order (the next column's "listbox" tab stop).
 */
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import { useApp, useAppDispatch } from "../../state/context";
import { useRovingFocus } from "../../hooks/useRovingFocus";
import { useViews } from "../../hooks/useViews";
import { Column, type MillerColumnItem } from "./Column";
import type { ContextualView } from "../../api/types";

/**
 * The kind of a parent object decides which view we request to
 * enumerate its children. Symbols → `call-graph`; everything else →
 * `overview` (which surfaces files / scopes / symbols depending on
 * the parent's kind).
 */
const CHILD_VIEW_BY_PARENT_KIND: Record<string, string> = {
  symbol: "call-graph",
  file: "overview",
  scope: "overview",
  workspace: "overview",
  module: "overview",
};

/**
 * Map the relations / children returned by a `ContextualView` into
 * the flat `MillerColumnItem[]` shape. The contextual view returns
 * `callers` / `callees` blocks for symbols with embedded
 * `RelationItem[]` arrays.
 */
function childrenFromView(
  view: ContextualView | undefined,
): MillerColumnItem[] {
  if (!view) return [];
  const out: MillerColumnItem[] = [];
  for (const block of view.blocks) {
    if (block.id === "callers" || block.id === "callees") {
      // body is CallListBlockBody { count, items: RelationItem[] }
      const body = block.body as {
        items?: Array<{
          object_id: string;
          name: string;
          kind: string;
          file: string;
          line: number;
        }>;
      };
      for (const it of body.items ?? []) {
        out.push({
          id: it.object_id,
          name: it.name,
          kind: it.kind,
          expandable: true,
          subtitle: `${it.file}:${it.line}`,
        });
      }
    }
  }
  // De-dupe by id, preserving order.
  const seen = new Set<string>();
  const unique: MillerColumnItem[] = [];
  for (const it of out) {
    if (seen.has(it.id)) continue;
    seen.add(it.id);
    unique.push(it);
  }
  return unique;
}

// ============================================================================
// ChildColumn — fetches children for ONE column
// ============================================================================

interface ChildColumnProps {
  index: number;
  parentObjectId: string;
  parentKind: string;
  parentLabel: string;
  parentViewId: string;
  isLeaf: boolean;
  autoFocus: boolean;
  onItemCountChange: (count: number) => void;
  onItemsResolved: (items: MillerColumnItem[]) => void;
  onError: (err: Error | null) => void;
  /**
   * Refs from the parent's roving-focus API. The column wires the
   * active item's `tabIndex` to the parent's `activeIndex` so Tab
   * between columns "just works".
   */
  activeIndex: number;
  onItemClick: (index: number) => void;
}

/**
 * A single column that fetches its children. Keeps the SWR call
 * local to the column so the parent does not have to know about
 * the view id → kind mapping.
 */
function ChildColumn({
  index,
  parentObjectId,
  parentKind,
  parentLabel,
  parentViewId,
  isLeaf,
  autoFocus,
  onItemCountChange,
  onItemsResolved,
  onError,
  activeIndex,
  onItemClick,
}: ChildColumnProps) {
  const childViewId =
    parentViewId ?? CHILD_VIEW_BY_PARENT_KIND[parentKind] ?? "overview";

  const { data, isLoading, isValidating, error } = useViews(
    parentObjectId,
    childViewId,
  );
  // SWR's `error` is typed `ApiError | undefined`; LoadingTier
  // expects `Error | null` so we coerce.
  const columnError = error ?? null;

  // Memo the items per render so the parent only re-receives them
  // when the underlying view changes.
  const items = useMemo(() => childrenFromView(data), [data]);

  useEffect(() => {
    onItemsResolved(items);
  }, [items, onItemsResolved]);

  useEffect(() => {
    onError(columnError);
  }, [columnError, onError]);

  useEffect(() => {
    onItemCountChange(items.length);
  }, [items.length, onItemCountChange]);

  // Build a local roving-focus handle. The listbox is interactive
  // only when this is the leaf column. The local column owns its
  // own activeIndex — the parent's `activeIndex` prop is treated
  // as a RESET signal (used when a new column is pushed).
  const localRoving = useRovingFocus(
    {
      itemCount: items.length,
      interactive: isLeaf,
      onActivate: (i) => onItemClick(i),
      onSelect: (i) => onItemClick(i),
      onCollapse: () => onItemClick(-1),
    },
    `${parentLabel} (column ${index + 1})`,
  );

  // If the parent signals a reset (activeIndex prop changed), push it
  // to the local roving. We only do this on prop change, not on every
  // render — otherwise local user interactions would be clobbered.
  const lastResetRef = useRef(activeIndex);
  useEffect(() => {
    if (activeIndex !== lastResetRef.current) {
      lastResetRef.current = activeIndex;
      localRoving.setActiveIndex(activeIndex);
    }
  }, [activeIndex, localRoving]);

  return (
    <Column
      index={index}
      label={parentLabel}
      items={items}
      isLoading={isLoading}
      isValidating={isValidating}
      error={columnError}
      isActive={isLeaf}
      roving={localRoving}
      autoFocus={isLeaf && autoFocus}
      onItemCountChange={() => {
        // count is already wired via useEffect above
      }}
    />
  );
}

// ============================================================================
// MillerColumns — the container
// ============================================================================

export interface MillerColumnsProps {
  /** Optional workspace label used in the empty state. */
  workspaceLabel?: string;
}

/**
 * Container. Reads `state.columns` and renders one `ChildColumn`
 * per entry. The active (last) column drives the keyboard contract
 * via a single `useRovingFocus` instance owned here at the top.
 */
export function MillerColumns({ workspaceLabel = "Workspace" }: MillerColumnsProps) {
  const { state } = useApp();
  const dispatch = useAppDispatch();
  const { columns } = state;

  // ---------------------------------------------------------------------
  // Shared roving state for the ACTIVE column
  // ---------------------------------------------------------------------
  // We track the index of the focused item across all columns so
  // the active column always picks up where the user left off.
  const [activeIndex, setActiveIndex] = useState(0);
  // We keep a map of items per column index so the active column
  // can use the right list. The active column's items drive the
  // roving focus.
  const itemsByColumnRef = useRef<Map<number, MillerColumnItem[]>>(new Map());

  // ---------------------------------------------------------------------
  // Reducer wiring
  // ---------------------------------------------------------------------
  const pushColumn = useCallback(
    (item: MillerColumnItem) => {
      const newCol = {
        object_id: item.id,
        active_view:
          CHILD_VIEW_BY_PARENT_KIND[item.kind] ?? "overview",
        kind: item.kind,
      };
      dispatch({ type: "PUSH_COLUMN", payload: newCol });
      setActiveIndex(0);
    },
    [dispatch],
  );

  const popColumn = useCallback(() => {
    if (columns.length === 0) return;
    const idx = columns.length - 1;
    dispatch({ type: "POP_COLUMN", payload: { index: idx } });
    setActiveIndex(0);
  }, [dispatch, columns.length]);

  const selectObject = useCallback(
    (item: MillerColumnItem) => {
      dispatch({
        type: "SELECT_OBJECT",
        payload: { objectId: item.id, viewId: "overview", kind: item.kind },
      });
    },
    [dispatch],
  );

  /**
   * Dispatched by the roving focus API. Maps the local index to the
   * active column's items and calls the right action.
   */
  const handleItemAction = useCallback(
    (index: number) => {
      if (index < 0) {
        // -1 signals "collapse" from the local roving API.
        popColumn();
        return;
      }
      const activeCol = columns.length - 1;
      const items = itemsByColumnRef.current.get(activeCol) ?? [];
      const item = items[index];
      if (!item) return;
      // ArrowRight (activate) pushes a new column; Enter (select)
      // triggers inspection. We treat them the same here because the
      // parent column is the active one and we want a "drill-in"
      // behaviour — the Object Inspector comes alive via the
      // SELECT_OBJECT action.
      if (item.expandable) {
        pushColumn(item);
      } else {
        selectObject(item);
      }
    },
    [columns.length, popColumn, pushColumn, selectObject],
  );

  // ---------------------------------------------------------------------
  // Empty state
  // ---------------------------------------------------------------------
  if (columns.length === 0) {
    return (
      <div
        data-testid="miller-columns-empty"
        role="region"
        aria-label="Miller columns navigator"
        className="flex h-full items-center justify-center p-6 text-center text-sm"
        style={{ color: "var(--color-text-secondary)" }}
      >
        <div>
          <p
            className="font-semibold"
            style={{ color: "var(--color-text-primary)" }}
          >
            {workspaceLabel}
          </p>
          <p className="mt-1 text-xs">
            Use the Spotter (Ctrl/Cmd+K) or pick an object to begin.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div
      data-testid="miller-columns"
      role="region"
      aria-label="Miller columns navigator"
      className="flex h-full overflow-x-auto"
    >
      {columns.map((col, idx) => {
        const isLeaf = idx === columns.length - 1;
        // We do not have a separate `kind` per column in the
        // reducer state — derive a sensible label from the object id.
        const lastSegment = col.object_id.split(":").slice(-1)[0] ?? col.object_id;
        const label = isLeaf
          ? "Active"
          : (lastSegment ?? "Column");
        return (
          <ChildColumn
            key={`${col.object_id}-${idx}`}
            index={idx}
            parentObjectId={col.object_id}
            parentKind={col.kind ?? "symbol"}
            parentLabel={label}
            parentViewId={
              col.active_view ??
              CHILD_VIEW_BY_PARENT_KIND[col.kind ?? "symbol"] ??
              "overview"
            }
            isLeaf={isLeaf}
            autoFocus={isLeaf}
            activeIndex={isLeaf ? activeIndex : -1}
            onItemCountChange={(count) => {
              if (isLeaf && activeIndex >= count) setActiveIndex(0);
            }}
            onItemsResolved={(items) => {
              itemsByColumnRef.current.set(idx, items);
              if (isLeaf && activeIndex >= items.length) setActiveIndex(0);
            }}
            onError={() => {
              /* surfaces via Column's LoadingTier */
            }}
            onItemClick={isLeaf ? handleItemAction : () => {}}
          />
        );
      })}
      {/* The container-level live region for global announcements
          (e.g. "Opened column for X"). */}
      <p
        ref={(el) => {
          // Bound to the active column's roving API via Column's
          // own live region; this one is a backup for messages that
          // are not column-scoped.
          if (el && el.textContent === "") return;
        }}
        aria-live="polite"
        aria-atomic="true"
        className="sr-only"
      />
    </div>
  );
}
