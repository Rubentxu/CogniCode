/**
 * `useLensResult` — apply a lens to an object.
 *
 * Endpoint: `GET /api/objects/:object_id/lenses/:lens_id`
 *
 * The hook takes a third arg so a `target` query string can be
 * appended (`?target=...`) for lenses that scope by file/symbol.
 * The result is a `LensResult` with `findings` and a one-line
 * `summary`.
 */
import useSWR from "swr";

import { lensResultSchema } from "../api/schemas";
import { ApiError, makeSwrFetcher } from "../api/client";
import type { LensResult } from "../api/types";

const lensResultFetcher = makeSwrFetcher(lensResultSchema);

export type UseLensResultArgs = {
  objectId: string | null;
  lensId: string | null;
  /** Optional target scope (file / symbol id). Encoded into `?target=`. */
  target?: string;
};

export function useLensResult({
  objectId,
  lensId,
  target,
}: UseLensResultArgs) {
  return useSWR<LensResult, ApiError>(
    objectId && lensId
      ? [
          `/objects/${encodeURIComponent(objectId)}/lenses/${encodeURIComponent(lensId)}/apply`,
          target ? { target } : undefined,
        ]
      : null,
    lensResultFetcher,
  );
}
