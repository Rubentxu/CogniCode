/**
 * `useWizardDraft` — localStorage auto-save/restore for the ViewSpecWizard.
 *
 * Design (ADR-008 §Draft Persistence):
 * - Key: `viewspec-draft-{objectId}` — per-object scoping matches the
 *   per-object nature of the wizard (each inspected object gets its own draft).
 * - Auto-save: debounced 1s after every wizard state change.
 * - Restore: on wizard open if a draft exists for that object.
 * - Clear: on explicit save or cancel.
 * - LRU cap: 20 drafts max; evict the oldest on overflow (by `savedAt` timestamp).
 *
 * The draft stores the serialisable wizard state plus enough metadata to
 * support ownership-aware pre-fill in the wizard.
 */
import { useCallback, useEffect, useRef, useState } from "react";

import type { ViewSpec } from "../api/schemas";

// ============================================================================
// Types
// ============================================================================

/**
 * The subset of wizard state that is worth persisting.
 * Excludes ephemeral UI state (step, loading, error).
 */
export interface WizardDraftState {
  viewKind: ViewKind | null;
  rendererKind: RendererKind | null;
  query: string;
  transformKind: "none" | "jsonata";
  jsonataExpression: string;
  title: string;
}

export type ViewKind = import("../api/schemas").ViewKind;
export type RendererKind = import("../api/schemas").RendererKind;

/** Full persisted draft including metadata. */
export interface WizardDraft {
  /** The object this draft belongs to. */
  objectId: string;
  /** Serialised wizard state. */
  state: WizardDraftState;
  /** When the draft was last saved (ISO string). */
  savedAt: string;
  /** The draft's original ViewSpec id if this is an edit draft. */
  editSpecId?: string;
}

interface UseWizardDraftOptions {
  /** Object being inspected — determines the localStorage key. */
  objectId: string;
  /** If provided, the draft is for editing this existing spec. */
  editSpec?: ViewSpec;
  /** Called with the restored state when a draft is found. */
  onRestore?: (state: WizardDraftState) => void;
}

interface UseWizardDraftResult {
  /** Current draft state (null if no draft exists). */
  draft: WizardDraft | null;
  /** Persist the current wizard state as a draft. */
  save: (state: WizardDraftState) => void;
  /** Clear the draft for this object (on save or cancel). */
  clear: () => void;
  /** Whether there is an active draft for this object. */
  hasDraft: boolean;
}

// ============================================================================
// Constants
// ============================================================================

const DRAFT_KEY_PREFIX = "viewspec-draft-";
const DEBOUNCE_MS = 1_000;
const MAX_DRAFTS = 20;

// ============================================================================
// localStorage helpers
// ============================================================================

/**
 * Get the localStorage key for a given object id.
 */
function draftKey(objectId: string): string {
  return `${DRAFT_KEY_PREFIX}${objectId}`;
}

/**
 * Parse a draft from localStorage. Returns null if absent or corrupt.
 */
function loadDraft(objectId: string): WizardDraft | null {
  try {
    const raw = localStorage.getItem(draftKey(objectId));
    if (!raw) return null;
    return JSON.parse(raw) as WizardDraft;
  } catch {
    return null;
  }
}

/**
 * Save a draft to localStorage.
 */
function persistDraft(draft: WizardDraft): void {
  try {
    localStorage.setItem(draftKey(draft.objectId), JSON.stringify(draft));
  } catch {
    // localStorage may be full — evict oldest drafts and retry once.
    evictOldestDrafts(5);
    try {
      localStorage.setItem(draftKey(draft.objectId), JSON.stringify(draft));
    } catch {
      // Give up — drafts are best-effort.
    }
  }
}

/**
 * Remove the draft for a given object id.
 */
function removeDraft(objectId: string): void {
  try {
    localStorage.removeItem(draftKey(objectId));
  } catch {
    // Best-effort.
  }
}

/**
 * Evict the N oldest drafts by `savedAt` timestamp.
 * Called when localStorage is near capacity.
 */
function evictOldestDrafts(count: number): void {
  try {
    const all: Array<{ key: string; draft: WizardDraft }> = [];
    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i);
      if (key?.startsWith(DRAFT_KEY_PREFIX)) {
        try {
          const raw = localStorage.getItem(key);
          if (raw) {
            const draft = JSON.parse(raw) as WizardDraft;
            all.push({ key, draft });
          }
        } catch {
          // Skip corrupt entries.
        }
      }
    }
    all.sort((a, b) => a.draft.savedAt.localeCompare(b.draft.savedAt));
    const toEvict = all.slice(0, count);
    for (const { key } of toEvict) {
      localStorage.removeItem(key);
    }
  } catch {
    // Best-effort.
  }
}

/**
 * Enforce the 20-draft LRU cap.
 * After every successful persist, call this to trim overflow.
 */
function enforceLruCap(): void {
  try {
    const all: Array<{ key: string; draft: WizardDraft }> = [];
    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i);
      if (key?.startsWith(DRAFT_KEY_PREFIX)) {
        try {
          const raw = localStorage.getItem(key);
          if (raw) {
            const draft = JSON.parse(raw) as WizardDraft;
            all.push({ key, draft });
          }
        } catch {
          // Skip corrupt entries.
        }
      }
    }
    if (all.length <= MAX_DRAFTS) return;
    // Sort by savedAt ascending (oldest first) and remove the excess.
    all.sort((a, b) => a.draft.savedAt.localeCompare(b.draft.savedAt));
    const excess = all.length - MAX_DRAFTS;
    for (let i = 0; i < excess; i++) {
      localStorage.removeItem(all[i]!.key);
    }
  } catch {
    // Best-effort.
  }
}

// ============================================================================
// Hook
// ============================================================================

/**
 * `useWizardDraft` — manages localStorage auto-save/restore for a single
 * object-scoped wizard draft.
 *
 * Usage:
 * ```
 * const { draft, save, clear, hasDraft } = useWizardDraft({
 *   objectId,
 *   editSpec,
 *   onRestore: (state) => dispatch({ type: "RESTORE", payload: state }),
 * });
 * ```
 */
export function useWizardDraft({
  objectId,
  editSpec,
  onRestore,
}: UseWizardDraftOptions): UseWizardDraftResult {
  const [draft, setDraft] = useState<WizardDraft | null>(() => loadDraft(objectId));
  const debounceTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const onRestoreRef = useRef(onRestore);
  // Keep the callback ref fresh without causing re-renders.
  useEffect(() => {
    onRestoreRef.current = onRestore;
  }, [onRestore]);

  // Attempt to restore the draft when the hook mounts or objectId changes.
  useEffect(() => {
    const found = loadDraft(objectId);
    setDraft(found);
    if (found) {
      onRestoreRef.current?.(found.state);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [objectId]);

  /**
   * Persist a draft (debounced 1s).
   */
  const save = useCallback(
    (state: WizardDraftState) => {
      if (debounceTimerRef.current !== null) {
        clearTimeout(debounceTimerRef.current);
      }
      debounceTimerRef.current = setTimeout(() => {
        debounceTimerRef.current = null;
        const next: WizardDraft = {
          objectId,
          state,
          savedAt: new Date().toISOString(),
          editSpecId: editSpec?.id,
        };
        persistDraft(next);
        enforceLruCap();
        setDraft(next);
      }, DEBOUNCE_MS);
    },
    [objectId, editSpec?.id],
  );

  /**
   * Clear the draft for the current object.
   */
  const clear = useCallback(() => {
    if (debounceTimerRef.current !== null) {
      clearTimeout(debounceTimerRef.current);
      debounceTimerRef.current = null;
    }
    removeDraft(objectId);
    setDraft(null);
  }, [objectId]);

  // Cleanup on unmount.
  useEffect(() => {
    return () => {
      if (debounceTimerRef.current !== null) {
        clearTimeout(debounceTimerRef.current);
      }
    };
  }, []);

  return {
    draft,
    save,
    clear,
    hasDraft: draft !== null,
  };
}
