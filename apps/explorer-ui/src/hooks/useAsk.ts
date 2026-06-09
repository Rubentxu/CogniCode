/**
 * `useAsk` — dispatch hook for the contextual-help suggestion strip.
 *
 * Clicking a prompt in `SuggestionStrip` flows through this hook.
 * It substitutes the focused object's `{label}` and `{id}` into the
 * prompt's `params`, then routes to one of four handlers:
 *
 *   - `cognicode_ask`             → SWR mutation posting to `/api/ask`
 *   - `explorer_inspect_object`   → SELECT_OBJECT for the resolved
 *                                   object id
 *   - `explorer_get_view`         → SELECT_OBJECT with the resolved
 *                                   `view_id` and the focused object id
 *   - `explorer_open_workspace`   → `openWorkspace()` + SET_WORKSPACE
 *
 * If no object is focused, `dispatch` is a no-op (the user cannot
 * pick a prompt without an object, but the hook stays defensive in
 * case the strip is rendered with stale state).
 *
 * The hook returns `{ dispatch, isDispatching }`. `isDispatching`
 * tracks the `cognicode_ask` SWR mutation so the UI can show a
 * pending indicator on the pill that was clicked.
 */
import { useCallback } from "react";
import useSWRMutation from "swr/mutation";

import { apiPost } from "../api/client";
import { askResponseSchema, type AskResponse } from "../api/schemas";
import { useAppDispatch } from "../state/context";
import { openWorkspace } from "./useWorkspace";
import {
  substituteParams,
  type SuggestedQuestion,
} from "../config/suggestedQuestions";

// ---------------------------------------------------------------------------
// Hook signature
// ---------------------------------------------------------------------------

export type UseAskArgs = {
  /** The focused object's id. When `null`, dispatch is a no-op. */
  objectId: string | null;
  /** The focused object's label, used for `{label}` substitution. */
  objectLabel: string | null;
};

export type UseAskResult = {
  dispatch: (question: SuggestedQuestion) => void;
  isDispatching: boolean;
};

// ---------------------------------------------------------------------------
// SWR fetcher (per-call arg shape: { question: string })
// ---------------------------------------------------------------------------

/**
 * `useSWRMutation` requires the fetcher to be a stable function
 * reference for the duration of the call. We declare it at module
 * scope so the identity never changes; SWR passes `(key, { arg })`
 * and we forward `arg` as the POST body.
 */
async function askFetcher(
  _key: string,
  { arg }: { arg: { question: string } },
): Promise<AskResponse> {
  return apiPost("/ask", arg, askResponseSchema);
}

export function useAsk({ objectId, objectLabel }: UseAskArgs): UseAskResult {
  const dispatch = useAppDispatch();
  // `useSWRMutation` exposes a stable `trigger` plus `isMutating`. We
  // only invoke `trigger` on a `cognicode_ask` click; the other tool
  // types skip the network entirely.
  const { trigger: askTrigger, isMutating: isDispatching } = useSWRMutation(
    "/ask",
    askFetcher,
  );

  const dispatchQuestion = useCallback(
    (question: SuggestedQuestion): void => {
      if (!objectId) return;

      const substituted = substituteParams(question.params, {
        label: objectLabel ?? "",
        id: objectId,
      });

      switch (question.tool) {
        case "cognicode_ask": {
          const questionText = substituted.question;
          if (questionText && questionText.length > 0) {
            void askTrigger({ question: questionText });
          }
          return;
        }
        case "explorer_inspect_object": {
          const targetId = substituted.object_id ?? objectId;
          dispatch({
            type: "SELECT_OBJECT",
            payload: { objectId: targetId, viewId: "overview" },
          });
          return;
        }
        case "explorer_get_view": {
          const viewId = substituted.view_id ?? "overview";
          dispatch({
            type: "SELECT_OBJECT",
            payload: { objectId, viewId },
          });
          return;
        }
        case "explorer_open_workspace": {
          const rootPath = substituted.root_path;
          if (!rootPath) return;
          void openWorkspace(rootPath).then((summary) => {
            dispatch({ type: "SET_WORKSPACE", payload: summary });
          });
          return;
        }
      }
    },
    [objectId, objectLabel, askTrigger, dispatch],
  );

  return { dispatch: dispatchQuestion, isDispatching };
}
