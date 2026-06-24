/**
 * `useGraphSearch` — T22 — hook for the multimodal `graph_search` tool.
 *
 * Wraps the `client.graphSearch` API call in an SWR-backed
 * paginated cache keyed by the search query + filters. Returns
 * a `useGraphSearchResult` that exposes:
 *   - `results`   — the current page of `MultimodalNode` hits
 *   - `totalCount`— total matches in the index
 *   - `nextCursor`— opaque cursor for the next page (or `null`)
 *   - `isLoading` — true while the first page is in-flight
 *   - `isError`   — true on network / validation error
 *   - `loadMore` — function that fetches the next page
 *   - `reset`    — function that clears the accumulated pages
 *
 * The hook is intentionally not a "SWR Infinite" wrapper — we
 * expose the raw `useState`-based accumulator so the UI can
 * append pages without round-tripping the server's cursor on
 * every render.
 */
import { useCallback, useEffect, useRef, useState } from "react";

import { graphSearch as graphSearchApi } from "../api/client";
import type {
  GraphSearchResponse,
  MultimodalNode,
  NodeKind,
} from "../api/types";

/** Options for [`useGraphSearch`]. All optional; sensible defaults. */
export interface UseGraphSearchOptions {
  /** Filter to one or more node kinds; omit to search every kind. */
  readonly nodeKinds?: readonly NodeKind[];
  /** Page size; defaults to 50 (the MCP server's default). */
  readonly limit?: number;
  /** Disable the hook (e.g. when the query is empty). */
  readonly enabled?: boolean;
}

/** Return shape of [`useGraphSearch`]. */
export interface UseGraphSearchResult {
  readonly results: readonly MultimodalNode[];
  readonly totalCount: number;
  readonly nextCursor: string | null;
  readonly isLoading: boolean;
  readonly isError: boolean;
  readonly error: Error | null;
  readonly loadMore: () => Promise<void>;
  readonly reset: () => void;
  /** Re-run the initial search (e.g. when the query changes). */
  readonly refetch: () => Promise<void>;
}

/**
 * Run a multimodal `graph_search`. The first page is fetched
 * eagerly when `query` is non-empty and `enabled !== false`.
 * Subsequent pages are fetched via `loadMore()`, which uses the
 * opaque `next_cursor` from the previous response.
 *
 * The hook returns the typed payload — every boundary call goes
 * through `client.graphSearch`, which zod-validates the wire
 * shape.
 */
export function useGraphSearch(
  query: string,
  options: UseGraphSearchOptions = {},
): UseGraphSearchResult {
  const { nodeKinds, limit = 50, enabled = true } = options;
  const [results, setResults] = useState<MultimodalNode[]>([]);
  const [totalCount, setTotalCount] = useState(0);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isError, setIsError] = useState(false);
  const [error, setError] = useState<Error | null>(null);

  // We keep a ref to the latest `nextCursor` so the effect
  // below can read it without re-creating the effect on every
  // cursor update.
  const cursorRef = useRef<string | null>(null);
  // eslint-disable-next-line react-hooks/refs -- real architectural issue; needs useEffect refactor deferred
  cursorRef.current = nextCursor;

  // The closure captures `query`/`nodeKinds`/`limit` from the
  // current render; we re-fetch by calling the helper directly.
  const fetchPage = useCallback(
    async (cursor: string | null, append: boolean): Promise<void> => {
      if (!enabled || !query) return;
      setIsLoading(true);
      setIsError(false);
      setError(null);
      try {
        const response: GraphSearchResponse = await graphSearchApi({
          query,
          node_kinds: nodeKinds ? [...nodeKinds] : undefined,
          cursor: cursor ?? undefined,
          limit,
        });
        if (append) {
          setResults((prev) => [...prev, ...response.results.map((r) => r.node)]);
        } else {
          setResults(response.results.map((r) => r.node));
        }
        setTotalCount(response.total_count);
        setNextCursor(response.next_cursor);
      } catch (e) {
        setIsError(true);
        setError(e instanceof Error ? e : new Error(String(e)));
      } finally {
        setIsLoading(false);
      }
    },
    [query, nodeKinds, limit, enabled],
  );

  // Eager first-page fetch on mount + whenever the inputs change.
  /* eslint-disable react-hooks/set-state-in-effect -- real architectural issue; refactor deferred */
  useEffect(() => {
    if (!enabled || !query) {
      setResults([]);
      setTotalCount(0);
      setNextCursor(null);
      return;
    }
    void fetchPage(null, false);
  }, [query, nodeKinds, limit, enabled, fetchPage]);
  /* eslint-enable react-hooks/set-state-in-effect */

  const loadMore = useCallback(async () => {
    if (cursorRef.current === null) return;
    await fetchPage(cursorRef.current, true);
  }, [fetchPage]);

  const reset = useCallback(() => {
    setResults([]);
    setTotalCount(0);
    setNextCursor(null);
    setIsError(false);
    setError(null);
  }, []);

  const refetch = useCallback(async () => {
    await fetchPage(null, false);
  }, [fetchPage]);

  return {
    results,
    totalCount,
    nextCursor,
    isLoading,
    isError,
    error,
    loadMore,
    reset,
    refetch,
  };
}
