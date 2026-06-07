/**
 * `useRovingFocus` unit tests.
 *
 * The hook is a pure a11y / focus utility — we exercise it via
 * `renderHook` + a small keyboard event helper. The assertions cover
 * the full WAI-ARIA listbox keyboard contract.
 */
import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { type KeyboardEvent } from "react";

import { useRovingFocus, type RovingFocusOptions, type RovingFocusApi } from "./useRovingFocus";

type ResultRef = { current: RovingFocusApi };
type RerenderFn = (props: { opts: RovingFocusOptions; label?: string }) => void;
type HookHandle = { result: ResultRef; rerender: RerenderFn };

/**
 * Render the hook with a given options object. Returns a thin
 * handle so tests can access `result.current` and `rerender`.
 */
function setupHook(
  options: RovingFocusOptions,
  containerLabel?: string,
): HookHandle {
  const { result, rerender } = renderHook(
    ({ opts, label }: { opts: RovingFocusOptions; label?: string }) =>
      useRovingFocus(opts, label),
    {
      initialProps: { opts: options, label: containerLabel },
    },
  );
  return {
    result,
    // `rerender` from renderHook has a strict `label: string | undefined`
    // type that fights with the optional `label?: string` we use in
    // tests; we accept the cast here as the safest path through the
    // strict-generic surface of @testing-library/react.
    rerender: (newProps: { opts: RovingFocusOptions; label?: string }) => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      rerender(newProps as any);
    },
  };
}

function fireKey(
  result: ResultRef,
  key: string,
  opts: Partial<KeyboardEventInit> = {},
) {
  const { onKeyDown } = result.current.getContainerProps();
  act(() => {
    onKeyDown({
      key,
      preventDefault: () => {},
      ...opts,
    } as unknown as KeyboardEvent<HTMLUListElement>);
  });
}

describe("useRovingFocus", () => {
  it("starts at index 0", () => {
    const { result } = setupHook({ itemCount: 3, interactive: true });
    expect(result.current.activeIndex).toBe(0);
  });

  it("moves down on ArrowDown and wraps to first", () => {
    const { result } = setupHook({ itemCount: 3, interactive: true });
    fireKey(result, "ArrowDown");
    expect(result.current.activeIndex).toBe(1);
    fireKey(result, "ArrowDown");
    expect(result.current.activeIndex).toBe(2);
    fireKey(result, "ArrowDown");
    // Wraps to first (WAI-ARIA listbox spec) — this is intentional.
    expect(result.current.activeIndex).toBe(0);
  });

  it("moves up on ArrowUp and wraps to last", () => {
    const { result } = setupHook({ itemCount: 3, interactive: true });
    fireKey(result, "ArrowUp");
    expect(result.current.activeIndex).toBe(2);
  });

  it("jumps to first on Home and last on End", () => {
    const { result } = setupHook({ itemCount: 5, interactive: true });
    fireKey(result, "End");
    expect(result.current.activeIndex).toBe(4);
    fireKey(result, "Home");
    expect(result.current.activeIndex).toBe(0);
  });

  it("PageDown moves by 10", () => {
    const { result } = setupHook({ itemCount: 30, interactive: true });
    fireKey(result, "PageDown");
    expect(result.current.activeIndex).toBe(10);
  });

  it("PageUp moves by -10 and clamps via wrapping", () => {
    const { result } = setupHook({ itemCount: 30, interactive: true });
    fireKey(result, "PageUp");
    expect(result.current.activeIndex).toBe(20);
  });

  it("ArrowRight calls onActivate with current index", () => {
    const onActivate = vi.fn();
    const { result } = setupHook({
      itemCount: 3,
      interactive: true,
      onActivate,
    });
    fireKey(result, "ArrowDown");
    fireKey(result, "ArrowRight");
    expect(onActivate).toHaveBeenCalledWith(1);
  });

  it("ArrowLeft calls onCollapse", () => {
    const onCollapse = vi.fn();
    const { result } = setupHook({
      itemCount: 3,
      interactive: true,
      onCollapse,
    });
    fireKey(result, "ArrowLeft");
    expect(onCollapse).toHaveBeenCalledOnce();
  });

  it("Enter calls onSelect with the current index", () => {
    const onSelect = vi.fn();
    const { result } = setupHook({
      itemCount: 3,
      interactive: true,
      onSelect,
    });
    fireKey(result, "ArrowDown");
    fireKey(result, "ArrowDown");
    fireKey(result, "Enter");
    expect(onSelect).toHaveBeenCalledWith(2);
  });

  it("Space calls onSelect with the current index", () => {
    const onSelect = vi.fn();
    const { result } = setupHook({
      itemCount: 3,
      interactive: true,
      onSelect,
    });
    fireKey(result, " ");
    expect(onSelect).toHaveBeenCalledWith(0);
  });

  it("Escape calls onEscape", () => {
    const onEscape = vi.fn();
    const { result } = setupHook({
      itemCount: 3,
      interactive: true,
      onEscape,
    });
    fireKey(result, "Escape");
    expect(onEscape).toHaveBeenCalledOnce();
  });

  it("does not respond to keys when not interactive", () => {
    const onActivate = vi.fn();
    const { result } = setupHook({
      itemCount: 3,
      interactive: false,
      onActivate,
    });
    fireKey(result, "ArrowDown");
    expect(result.current.activeIndex).toBe(0);
    fireKey(result, "ArrowRight");
    expect(onActivate).not.toHaveBeenCalled();
  });

  it("clamps the index when itemCount shrinks", () => {
    const { result, rerender } = setupHook({ itemCount: 5, interactive: true });
    fireKey(result, "End");
    expect(result.current.activeIndex).toBe(4);
    rerender({ opts: { itemCount: 2, interactive: true }, label: undefined });
    expect(result.current.activeIndex).toBe(1);
  });

  it("sets the container role to listbox and tabIndex 0 when interactive", () => {
    const { result } = setupHook({ itemCount: 3, interactive: true });
    const props = result.current.getContainerProps();
    expect(props.role).toBe("listbox");
    expect(props.tabIndex).toBe(0);
  });

  it("sets the container tabIndex to -1 when not interactive", () => {
    const { result } = setupHook({ itemCount: 3, interactive: false });
    const props = result.current.getContainerProps();
    expect(props.tabIndex).toBe(-1);
  });

  it("getItemProps gives tabIndex 0 to the active item and -1 to others", () => {
    const { result } = setupHook({ itemCount: 3, interactive: true });
    fireKey(result, "ArrowDown");
    expect(result.current.getItemProps(0).tabIndex).toBe(-1);
    expect(result.current.getItemProps(1).tabIndex).toBe(0);
    expect(result.current.getItemProps(2).tabIndex).toBe(-1);
    expect(result.current.getItemProps(1)["aria-selected"]).toBe(true);
  });

  it("announce() writes to the live region ref", async () => {
    const { result } = setupHook({ itemCount: 3, interactive: true });
    const fake = document.createElement("p");
    // Wire the ref manually for the test.
    (
      result.current.liveRegionRef as { current: HTMLParagraphElement | null }
    ).current = fake;
    result.current.announce("Hello world");
    // announce uses queueMicrotask — wait one tick.
    await new Promise<void>((resolve) => {
      queueMicrotask(() => {
        expect(fake.textContent).toBe("Hello world");
        resolve();
      });
    });
  });

  it("setActiveIndex is clamped to the item count", () => {
    const { result } = setupHook({ itemCount: 3, interactive: true });
    act(() => result.current.setActiveIndex(10));
    expect(result.current.activeIndex).toBe(2);
    act(() => result.current.setActiveIndex(-5));
    expect(result.current.activeIndex).toBe(0);
  });

  it("ignores empty item lists on keypress", () => {
    const onActivate = vi.fn();
    const { result } = setupHook({
      itemCount: 0,
      interactive: true,
      onActivate,
    });
    fireKey(result, "ArrowDown");
    expect(result.current.activeIndex).toBe(0);
    fireKey(result, "ArrowRight");
    expect(onActivate).not.toHaveBeenCalled();
  });
});
