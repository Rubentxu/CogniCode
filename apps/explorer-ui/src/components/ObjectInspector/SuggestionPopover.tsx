/**
 * `SuggestionPopover` — small-viewport branch of the suggestion strip.
 *
 * On viewports < 900px the strip collapses to a single "What can I
 * do here?" trigger button. Clicking the button opens a native
 * `<dialog>` (`showModal()`) listing the same prompts the strip
 * would have shown. The dialog supports:
 *
 *   - Dismissal on Escape (native `cancel` event).
 *   - Dismissal on outside click (the `<dialog>` element itself
 *     receives the click; inner content clicks do not close).
 *   - Focus return to the trigger on close.
 *
 * Built on the native `<dialog>` (no Radix) — matches the existing
 * "no Radix" pattern in the codebase and gives us Escape + focus
 * trap for free.
 */
import {
  forwardRef,
  useImperativeHandle,
  useRef,
  useState,
} from "react";

import type { SuggestedQuestion } from "../../config/suggestedQuestions";

export interface SuggestionPopoverProps {
  prompts: readonly SuggestedQuestion[];
  onDispatch: (q: SuggestedQuestion) => void;
  /** Text on the trigger button. Defaults to "What can I do here?". */
  ariaLabel?: string;
  /** Optional: extra className on the strip wrapper. */
  className?: string;
}

export interface SuggestionPopoverHandle {
  open: () => void;
  close: () => void;
}

/**
 * Imperative ref handle. Lets a parent open/close the popover
 * without owning the trigger (e.g. global hotkey to show the help).
 */
export const SuggestionPopover = forwardRef<
  SuggestionPopoverHandle,
  SuggestionPopoverProps
>(function SuggestionPopover({ prompts, onDispatch, ariaLabel, className }, ref) {
  const triggerRef = useRef<HTMLButtonElement | null>(null);
  const dialogRef = useRef<HTMLDialogElement | null>(null);
  // `open` mirrors the dialog's `open` attribute. We track it as
  // React state so the close handlers can decide whether to refocus
  // the trigger. (The native `<dialog>` `open` property is also
  // mutated by `showModal` / `close`; reading from it is fine, but
  // we want a re-render trigger.)
  const [isOpen, setIsOpen] = useState(false);

  function openDialog(): void {
    const dlg = dialogRef.current;
    if (!dlg || dlg.open) return;
    dlg.showModal();
    setIsOpen(true);
  }

  function closeDialog(): void {
    const dlg = dialogRef.current;
    if (!dlg || !dlg.open) return;
    dlg.close();
    setIsOpen(false);
    // Return focus to the trigger for keyboard users.
    triggerRef.current?.focus();
  }

  useImperativeHandle(
    ref,
    () => ({
      open: openDialog,
      close: closeDialog,
    }),
    [],
  );

  function handleCancel(e: React.SyntheticEvent<HTMLDialogElement>): void {
    // Native `cancel` fires on Escape. Prevent default so we can
    // restore focus before the dialog disappears; React then calls
    // `close()` ourselves.
    e.preventDefault();
    closeDialog();
  }

  function handleBackdropClick(e: React.MouseEvent<HTMLDialogElement>): void {
    // The native dialog backdrop is the dialog element itself; a
    // click on inner content bubbles up but `e.target` is the
    // innermost element. We close only when the user clicked the
    // dialog element directly (the backdrop area).
    if (e.target === dialogRef.current) {
      closeDialog();
    }
  }

  function handlePromptClick(prompt: SuggestedQuestion): void {
    onDispatch(prompt);
    closeDialog();
  }

  return (
    <div className={className} data-testid="suggestion-popover">
      <button
        ref={triggerRef}
        type="button"
        data-testid="suggestion-popover-trigger"
        aria-haspopup="dialog"
        aria-expanded={isOpen}
        onClick={openDialog}
        className="rounded-full px-3 py-1 text-xs"
        style={{
          backgroundColor: "var(--color-surface-overlay)",
          color: "var(--color-text-primary)",
        }}
      >
        {ariaLabel ?? "What can I do here?"}
      </button>
      <dialog
        ref={dialogRef}
        data-testid="suggestion-popover-dialog"
        aria-label={ariaLabel ?? "What can I do here?"}
        onCancel={handleCancel}
        onClick={handleBackdropClick}
        className="rounded-md p-4 text-sm"
        style={{
          backgroundColor: "var(--color-surface)",
          color: "var(--color-text-primary)",
          border: "1px solid var(--color-border)",
          minWidth: "20rem",
        }}
      >
        <ul role="list" className="m-0 flex list-none flex-col gap-2 p-0">
          {prompts.map((prompt) => (
            <li key={prompt.id}>
              <button
                type="button"
                data-testid={`suggestion-popover-item-${prompt.id}`}
                onClick={() => handlePromptClick(prompt)}
                disabled={prompt.requiresGraph /* see strip: stale gating */}
                className="w-full rounded px-3 py-2 text-left text-sm"
                style={{
                  backgroundColor: "var(--color-surface-overlay)",
                  color: "var(--color-text-primary)",
                }}
              >
                {prompt.label}
              </button>
            </li>
          ))}
        </ul>
      </dialog>
    </div>
  );
});
