/**
 * `globalShortcut` — the single owner of the global shortcut rule.
 *
 * Two things live here, and nothing else does:
 *
 * 1. **The decision** — `decideShortcut` is a pure function: given a
 *    key, whether a modifier is held, and whether the keystroke
 *    happened inside a text field, it says what the shortcut means.
 *    No DOM, no clock, no side effect.
 * 2. **The effect** — `installGlobalShortcut` registers the keydown
 *    listener and, in the very same call, publishes the availability
 *    signal on `<html>`. The disposer unregisters the listener and
 *    retracts the signal.
 *
 * The signal exists because the E2E suite used to wait for the
 * listener with a fixed clock. A clock is a guess about how long the
 * application takes; the attribute is a fact about whether the
 * listener is registered at all. Because both happen inside
 * `installGlobalShortcut` and are undone together, the attribute is
 * present **exactly when** the shortcut answers — it can neither
 * anticipate a listener that is not there nor outlive one that is
 * gone.
 *
 * The attribute name and its ready value are part of a published
 * contract: the E2E suite imports them from here instead of
 * repeating the literal, so there is a single owner for the rule.
 */

/**
 * Attribute set on `<html>` while the global shortcut is registered.
 * Tests read it; nothing in the application branches on it.
 */
export const GLOBAL_SHORTCUT_ATTRIBUTE = "data-cognicode-global-shortcut";

/** Value of {@link GLOBAL_SHORTCUT_ATTRIBUTE} while the shortcut is registered. */
export const GLOBAL_SHORTCUT_READY = "ready";

/** What a keystroke means for the global shortcut. */
export type ShortcutAction = "toggle" | "open" | "none";

/** The facts a keystroke is decided from. */
export interface ShortcutInput {
  /** `KeyboardEvent.key` — compared case-insensitively for `K`. */
  key: string;
  /** Whether the meta or control modifier was held. */
  modifier: boolean;
  /** Whether the keystroke landed inside an editable text field. */
  inTextField: boolean;
}

/**
 * Decide what a keystroke means. Pure: same input, same answer, no
 * effect.
 *
 * Cmd/Ctrl+K toggles the palette. `/` opens it — the convention
 * popularized by GitHub — but only when no modifier is held and the
 * keystroke did not land in a text field, where `/` is a literal
 * character the user is typing.
 */
export function decideShortcut(input: ShortcutInput): ShortcutAction {
  if (input.modifier && input.key.toLowerCase() === "k") return "toggle";
  if (input.key === "/" && !input.modifier && !input.inTextField) return "open";
  return "none";
}

/** Callbacks the shortcut dispatches to. */
export interface GlobalShortcutOptions {
  /** Called when the shortcut toggles the palette. */
  onToggle: () => void;
  /** Called when the shortcut opens the palette. */
  onOpen: () => void;
}

/**
 * Number of live installations. The signal is a fact about the
 * application, not about one component: it stays published while any
 * listener is registered, so two mounted owners cannot publish a
 * signal that outlives the shortcut it advertises.
 */
let activeInstalls = 0;

/**
 * Register the global shortcut and publish its availability signal.
 *
 * Returns the disposer. Calling it twice is a no-op the second time.
 */
export function installGlobalShortcut(options: GlobalShortcutOptions): () => void {
  function onKey(event: KeyboardEvent) {
    const action = decideShortcut({
      key: event.key,
      modifier: event.metaKey || event.ctrlKey,
      inTextField:
        event.target instanceof HTMLInputElement ||
        event.target instanceof HTMLTextAreaElement,
    });
    if (action === "none") return;
    event.preventDefault();
    if (action === "toggle") options.onToggle();
    else options.onOpen();
  }

  window.addEventListener("keydown", onKey);
  activeInstalls += 1;
  document.documentElement.setAttribute(
    GLOBAL_SHORTCUT_ATTRIBUTE,
    GLOBAL_SHORTCUT_READY,
  );

  let disposed = false;
  return () => {
    if (disposed) return;
    disposed = true;
    window.removeEventListener("keydown", onKey);
    activeInstalls -= 1;
    if (activeInstalls === 0) {
      document.documentElement.removeAttribute(GLOBAL_SHORTCUT_ATTRIBUTE);
    }
  };
}
