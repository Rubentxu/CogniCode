/**
 * `viewSpecWizard` slice — controls the open/close state of the
 * ViewSpecWizard modal so the trigger can live in the app header
 * (ShellLayout) while the wizard itself renders inside PaneInspector.
 *
 * Persisted in localStorage so the user's preference (whether they
 * left it open) survives reloads — same pattern as `lensSidebar`.
 */

export type ViewSpecWizardAction =
  | { type: "OPEN_VIEWSPEC_WIZARD" }
  | { type: "CLOSE_VIEWSPEC_WIZARD" }
  | { type: "TOGGLE_VIEWSPEC_WIZARD" };

export interface ViewSpecWizardState {
  open: boolean;
}

const STORAGE_KEY = "cognicode.viewSpecWizard.open";

function loadInitial(): boolean {
  if (typeof window === "undefined") return false;
  try {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    if (stored === "true") return true;
    if (stored === "false") return false;
  } catch {
    // ignore — SSR / disabled storage
  }
  return false;
}

function persist(open: boolean): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(STORAGE_KEY, String(open));
  } catch {
    // ignore
  }
}

export function viewSpecWizardReducer(
  state: ViewSpecWizardState = { open: loadInitial() },
  action: ViewSpecWizardAction,
): ViewSpecWizardState {
  switch (action.type) {
    case "OPEN_VIEWSPEC_WIZARD": {
      if (state.open) return state;
      persist(true);
      return { open: true };
    }
    case "CLOSE_VIEWSPEC_WIZARD": {
      if (!state.open) return state;
      persist(false);
      return { open: false };
    }
    case "TOGGLE_VIEWSPEC_WIZARD": {
      const open = !state.open;
      persist(open);
      return { open };
    }
    default:
      return state;
  }
}
