/**
 * `ViewSpecWizardTrigger` — header button to open the ViewSpecWizard.
 *
 * Lives in the app header (ShellLayout) so the wizard is discoverable
 * even before the user has navigated to a specific object. Disabled
 * (with a tooltip explaining why) when no object is active, because
 * the wizard requires an object context to determine `applies_to`.
 *
 * The wizard itself renders inside PaneInspector (where the object
 * is already resolved). Cross-component communication goes through
 * the global Zustand slice `viewSpecWizard`.
 */

import type { JSX } from "react";
import { useAppDispatch, useAppState } from "../state/context";

export function ViewSpecWizardTrigger(): JSX.Element {
  const dispatch = useAppDispatch();
  const { viewSpecWizard, activeObjectId } = useAppState();

  const isOpen = viewSpecWizard.open;
  const disabled = activeObjectId === null;

  return (
    <button
      type="button"
      data-testid="viewspec-wizard-trigger"
      aria-label={
        disabled
          ? "Custom view — select an object first"
          : isOpen
            ? "Close custom view wizard"
            : "Create custom view"
      }
      aria-pressed={isOpen}
      aria-disabled={disabled}
      disabled={disabled}
      onClick={() => {
        if (disabled) return;
        dispatch({ type: "TOGGLE_VIEWSPEC_WIZARD" });
      }}
      className="flex items-center gap-1.5 rounded-md px-2 py-1 text-xs"
      style={{
        backgroundColor: isOpen
          ? "var(--color-accent)"
          : "var(--color-surface-overlay)",
        color: isOpen
          ? "var(--color-accent-foreground)"
          : "var(--color-text-secondary)",
        opacity: disabled ? 0.5 : 1,
        cursor: disabled ? "not-allowed" : "pointer",
      }}
      title={
        disabled
          ? "Select an object first to create a custom view"
          : isOpen
            ? "Close custom view wizard"
            : "Create custom view"
      }
    >
      <svg
        width={14}
        height={14}
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        strokeWidth={1.5}
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden="true"
      >
        <path d="M8 2 L9 7 L14 8 L9 9 L8 14 L7 9 L2 8 L7 7 Z" />
      </svg>
      <span>{disabled ? "Custom View · select an object" : "Custom View"}</span>
    </button>
  );
}
