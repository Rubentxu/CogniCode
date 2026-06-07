/**
 * `useLenses` — list the lenses applicable to a given object.
 *
 * Endpoint: `GET /api/objects/:object_id/lenses`
 *
 * The backend filters by `applicable_types` server-side, so the
 * returned `LensDescriptor[]` is exactly what the user can run.
 */
import useSWR from "swr";
import { z } from "zod";

import { lensDescriptorSchema } from "../api/schemas";
import { ApiError, makeSwrFetcher } from "../api/client";
import type { LensDescriptor } from "../api/types";

const lensListSchema = z.array(lensDescriptorSchema);
type LensList = z.infer<typeof lensListSchema>;

const lensListFetcher = makeSwrFetcher(lensListSchema);

export function useLenses(
  objectId: string | null,
) {
  return useSWR<LensList, ApiError>(
    objectId ? `/objects/${encodeURIComponent(objectId)}/lenses` : null,
    lensListFetcher,
  );
}

export type { LensDescriptor, LensList };
