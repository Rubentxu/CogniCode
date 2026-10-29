/**
 * `globalShortcut.ts` tests — the availability signal of the global
 * shortcut.
 *
 * The invariant under test: the DOM attribute that advertises the
 * shortcut is present **exactly when** the keydown listener is
 * registered. A signal that can outlive — or anticipate — its
 * listener is a lie, and every E2E wait built on top of it inherits
 * the lie.
 */
import { describe, it, expect, vi, afterEach } from "vitest";

import {
  GLOBAL_SHORTCUT_ATTRIBUTE,
  GLOBAL_SHORTCUT_READY,
  decideShortcut,
  installGlobalShortcut,
} from "./globalShortcut";

function signal(): string | null {
  return document.documentElement.getAttribute(GLOBAL_SHORTCUT_ATTRIBUTE);
}

describe("decideShortcut", () => {
  it("maps the modifier + K combination to toggle", () => {
    expect(
      decideShortcut({ key: "k", modifier: true, inTextField: false }),
    ).toBe("toggle");
  });

  it("is case-insensitive for the K shortcut", () => {
    expect(
      decideShortcut({ key: "K", modifier: true, inTextField: false }),
    ).toBe("toggle");
  });

  it("maps the slash key to open", () => {
    expect(
      decideShortcut({ key: "/", modifier: false, inTextField: false }),
    ).toBe("open");
  });

  it("ignores the slash key inside a text field", () => {
    expect(
      decideShortcut({ key: "/", modifier: false, inTextField: true }),
    ).toBe("none");
  });

  it("ignores the slash key when a modifier is held", () => {
    expect(
      decideShortcut({ key: "/", modifier: true, inTextField: false }),
    ).toBe("none");
  });

  it("ignores every other key", () => {
    expect(
      decideShortcut({ key: "j", modifier: false, inTextField: false }),
    ).toBe("none");
    expect(
      decideShortcut({ key: "Enter", modifier: false, inTextField: false }),
    ).toBe("none");
  });
});

describe("installGlobalShortcut", () => {
  afterEach(() => {
    document.documentElement.removeAttribute(GLOBAL_SHORTCUT_ATTRIBUTE);
  });

  it("publishes the signal while the listener is registered", () => {
    const dispose = installGlobalShortcut({
      onToggle: vi.fn(),
      onOpen: vi.fn(),
    });

    expect(signal()).toBe(GLOBAL_SHORTCUT_READY);

    dispose();
  });

  it("retracts the signal when the listener is unregistered", () => {
    const dispose = installGlobalShortcut({
      onToggle: vi.fn(),
      onOpen: vi.fn(),
    });
    dispose();

    expect(signal()).toBeNull();
  });

  it("toggles on the modifier + K combination", () => {
    const onToggle = vi.fn();
    const onOpen = vi.fn();
    const dispose = installGlobalShortcut({ onToggle, onOpen });

    window.dispatchEvent(
      new KeyboardEvent("keydown", { key: "k", metaKey: true }),
    );

    expect(onToggle).toHaveBeenCalledTimes(1);
    expect(onOpen).not.toHaveBeenCalled();

    dispose();
  });

  it("opens on the slash key", () => {
    const onToggle = vi.fn();
    const onOpen = vi.fn();
    const dispose = installGlobalShortcut({ onToggle, onOpen });

    window.dispatchEvent(new KeyboardEvent("keydown", { key: "/" }));

    expect(onOpen).toHaveBeenCalledTimes(1);
    expect(onToggle).not.toHaveBeenCalled();

    dispose();
  });

  it("stops dispatching once the listener is unregistered", () => {
    const onToggle = vi.fn();
    const onOpen = vi.fn();
    const dispose = installGlobalShortcut({ onToggle, onOpen });
    dispose();

    window.dispatchEvent(
      new KeyboardEvent("keydown", { key: "k", metaKey: true }),
    );
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "/" }));

    expect(onToggle).not.toHaveBeenCalled();
    expect(onOpen).not.toHaveBeenCalled();
  });
});
