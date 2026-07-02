/**
 * `useAffordance` — fetches the affordance matrix for a given InspectableObjectType.
 *
 * Graceful degradation: returns an empty array for unknown types (backend returns 200).
 *
 * @param objectType - The InspectableObjectType to fetch affordances for
 *                    (pass `null` to skip the fetch)
 */
import useSWR from "swr";

import { ApiError, makeSwrFetcher } from "../api/client";
import { affordanceSchema, type Affordance } from "../api/affordanceSchema";
import { z } from "zod";

const affordancesFetcher = makeSwrFetcher(z.array(affordanceSchema));

export function useAffordance(objectType: string | null) {
  const key = objectType
    ? `/affordances/${encodeURIComponent(objectType)}`
    : null;

  return useSWR<Affordance[], ApiError>(key, affordancesFetcher, {
    revalidateOnFocus: false,
  });
}
