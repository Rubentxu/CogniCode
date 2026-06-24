/**
 * `lensSidebar` slice — controls the visibility of the right-side
 * LensPanel sidebar. Persisted in localStorage so the user's choice
 * survives reloads (matches the pattern in useWizardDraft).
 */

export type LensSidebarAction =
  | { type: "TOGGLE_LENS_SIDEBAR" }
  | { type: "SET_LENS_SIDEBAR"; payload: { open: boolean } };

export interface LensSidebarState {
  open: boolean;
}

const STORAGE_KEY = "cognicode.lensSidebar.open";

function loadInitial(): boolean {
  if (typeof window === "undefined") return false;
  try {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    if (stored === "true") return true;
    if (stored === "false") return false;
  } catch {
    // ignore
  }
  return false; // default collapsed
}

function persist(open: boolean): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(STORAGE_KEY, String(open));
  } catch {
    // ignore
  }
}

export function lensSidebarReducer(
  state: LensSidebarState = { open: loadInitial() },
  action: LensSidebarAction,
): LensSidebarState {
  switch (action.type) {
    case "TOGGLE_LENS_SIDEBAR": {
      const open = !state.open;
      persist(open);
      return { open };
    }
    case "SET_LENS_SIDEBAR": {
      persist(action.payload.open);
      return { open: action.payload.open };
    }
    default:
      return state;
  }
}
