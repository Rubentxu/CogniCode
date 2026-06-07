/**
 * `useRovingFocus` — roving tabindex for Miller Columns.
 *
 * Implements the WAI-ARIA "Treegrid" / "Listbox" keyboard contract:
 * - Tab / Shift+Tab: moves FOCUS between columns and adjacent regions
 *   (returns to the caller, which decides what "next region" means).
 * - ArrowUp / ArrowDown: moves within the active column.
 * - ArrowRight: expands the focused item (parent decides what expand
 *   means — typically open a child column).
 * - ArrowLeft: collapses the current column / returns focus to the
 *   parent column's focused item.
 * - Enter: activates the focused item (parent dispatches selection).
 * - Escape: closes the current column.
 * - Home / End: jumps to first / last item in the column.
 * - PageUp / PageDown: jumps by 10 items (matches WAI-ARIA listbox
 *   recommended behaviour).
 *
 * The hook is purely a focus/announcer utility — it does NOT know
 * about Miller Columns' state machine (push/pop column). It exposes
 * `announce` so the parent can inject its own messages ("Opened
 * column for `build_overview`" etc.).
 *
 * Announcements flow through an `aria-live="polite"` element wired
 * to the hook, so screen readers receive the navigation result.
 */
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type RefObject,
} from "react";

export type RovingFocusOptions = {
  /**
   * Total number of items in the active column. The hook clamps
   * navigation indices to `[0, itemCount - 1]`.
   */
  itemCount: number;
  /**
   * Whether the column has an "active item" concept — when true,
   * the hook will track the index of the focused item separately
   * from the DOM-active element. Set false for inert columns.
   */
  interactive: boolean;
  /**
   * Called when ArrowRight is pressed on a focused item — the parent
   * should open a child column and let the hook know via
   * `setActiveItemIndex`.
   */
  onActivate?: (index: number) => void;
  /**
   * Called when Enter is pressed — the parent should select the item
   * (dispatch SELECT_OBJECT to the reducer).
   */
  onSelect?: (index: number) => void;
  /**
   * Called when ArrowLeft is pressed and the user is on the FIRST
   * column — the parent should close the column / pop it.
   */
  onCollapse?: () => void;
  /**
   * Called when Escape is pressed — the parent should close / clear.
   */
  onEscape?: () => void;
  /**
   * Called on Home / End for focus jumps. Optional — the hook tracks
   * index locally if not provided.
   */
  onFocusIndexChange?: (index: number) => void;
};

export type RovingFocusApi = {
  /** The current focus index inside the column. */
  activeIndex: number;
  /** Set the focus index programmatically (e.g. when a new column is mounted). */
  setActiveIndex: (index: number) => void;
  /**
   * Wire this to the column's root `<ul role="listbox">` so the
   * hook can intercept keystrokes and update tabindex.
   */
  getContainerProps: () => {
    onKeyDown: (event: KeyboardEvent<HTMLUListElement>) => void;
    role: "listbox";
    tabIndex: number;
    "aria-label"?: string;
  };
  /**
   * Wire this to each `<li role="option">` so the active item gets
   * `tabIndex={0}` and others get `tabIndex={-1}`.
   *
   * `columnIndex` namespaces the generated DOM id so adjacent columns
   * do not collide on `miller-column-item-N`.
   */
  getItemProps: (index: number, columnIndex?: number) => {
    id: string;
    role: "option";
    tabIndex: number;
    "aria-selected"?: boolean;
    onClick: () => void;
  };
  /**
   * Ref to the live region (`<p aria-live="polite" />`) the hook
   * writes announcements into. Mount it inside the column.
   */
  liveRegionRef: RefObject<HTMLParagraphElement | null>;
  /**
   * Push a string into the live region. Use for navigation feedback
   * ("Opened symbol: build_overview", "Column collapsed", etc.).
   */
  announce: (message: string) => void;
};

/**
 * Roving tabindex state for a single column. Behaviour is shaped by
 * the WAI-ARIA listbox pattern — see
 * https://www.w3.org/WAI/ARIA/apg/patterns/listbox/.
 */
export function useRovingFocus(
  options: RovingFocusOptions,
  containerLabel?: string,
): RovingFocusApi {
  const {
    itemCount,
    interactive,
    onActivate,
    onSelect,
    onCollapse,
    onEscape,
    onFocusIndexChange,
  } = options;

  const [activeIndex, setActiveIndexState] = useState(0);
  const liveRegionRef = useRef<HTMLParagraphElement | null>(null);
  // Last valid item count, used to clamp on render.
  const lastItemCountRef = useRef(itemCount);

  useEffect(() => {
    lastItemCountRef.current = itemCount;
    // Clamp the active index when the item list shrinks. The
    // setState is intentional — it keeps the cursor in-bounds
    // after the parent shrinks the list, and the cascading render
    // is the whole point of the roving contract.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setActiveIndexState((prev) => {
      if (itemCount === 0) return 0;
      return Math.min(prev, itemCount - 1);
    });
  }, [itemCount]);

  const setActiveIndex = useCallback(
    (index: number) => {
      if (itemCount === 0) return;
      const clamped = Math.max(0, Math.min(index, itemCount - 1));
      setActiveIndexState(clamped);
      onFocusIndexChange?.(clamped);
    },
    [itemCount, onFocusIndexChange],
  );

  const announce = useCallback((message: string) => {
    // Wipe the live region first — some screen readers (NVDA + Firefox)
    // do not re-announce identical text back-to-back, so a blank
    // in-between guarantees the message is spoken.
    const el = liveRegionRef.current;
    if (!el) return;
    el.textContent = "";
    // Defer the assignment to a microtask so the DOM update lands
    // after the screen reader has flushed the previous text.
    queueMicrotask(() => {
      el.textContent = message;
    });
  }, []);

  const moveBy = useCallback(
    (delta: number) => {
      if (itemCount === 0) return;
      setActiveIndex((activeIndex + delta + itemCount) % itemCount);
    },
    [activeIndex, itemCount, setActiveIndex],
  );

  const onKeyDown = useCallback(
    (event: KeyboardEvent<HTMLUListElement>) => {
      if (!interactive || itemCount === 0) return;
      switch (event.key) {
        case "ArrowDown":
          event.preventDefault();
          moveBy(1);
          break;
        case "ArrowUp":
          event.preventDefault();
          moveBy(-1);
          break;
        case "Home":
          event.preventDefault();
          setActiveIndex(0);
          break;
        case "End":
          event.preventDefault();
          setActiveIndex(itemCount - 1);
          break;
        case "PageDown":
          event.preventDefault();
          moveBy(10);
          break;
        case "PageUp":
          event.preventDefault();
          moveBy(-10);
          break;
        case "ArrowRight":
          event.preventDefault();
          onActivate?.(activeIndex);
          break;
        case "ArrowLeft":
          event.preventDefault();
          onCollapse?.();
          break;
        case "Enter":
        case " ":
          event.preventDefault();
          onSelect?.(activeIndex);
          break;
        case "Escape":
          event.preventDefault();
          onEscape?.();
          break;
        default:
          // Let the keystroke bubble — no-op.
          break;
      }
    },
    [
      interactive,
      itemCount,
      moveBy,
      activeIndex,
      setActiveIndex,
      onActivate,
      onSelect,
      onCollapse,
      onEscape,
    ],
  );

  const getContainerProps: RovingFocusApi["getContainerProps"] = () => ({
    onKeyDown,
    role: "listbox",
    tabIndex: interactive ? 0 : -1,
    ...(containerLabel !== undefined ? { "aria-label": containerLabel } : {}),
  });

  const getItemProps: RovingFocusApi["getItemProps"] = (index, columnIndex) => {
    const isActive = index === activeIndex;
    // Namespace the id by column position so adjacent Miller columns
    // do not emit duplicate ids (invalid HTML, breaks label-for +
    // document.getElementById lookups).
    const col = columnIndex ?? 0;
    return {
      id: `miller-column-${col}-item-${index}`,
      role: "option" as const,
      tabIndex: isActive ? 0 : -1,
      "aria-selected": isActive,
      onClick: () => {
        setActiveIndex(index);
        onSelect?.(index);
      },
    };
  };

  return {
    activeIndex,
    setActiveIndex,
    getContainerProps,
    getItemProps,
    liveRegionRef,
    announce,
  };
}
