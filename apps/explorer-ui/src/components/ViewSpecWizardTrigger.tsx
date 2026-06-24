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
      aria-label={isOpen ? "Close custom view wizard" : "Create custom view"}
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
      <span aria-hidden="true">✦</span>
      <span>Custom View</span>
    </button>
  );
}
