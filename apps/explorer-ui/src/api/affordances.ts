/**
 * Fetch affordances for a given InspectableObjectType.
 *
 * Graceful degradation: the backend returns 200 with an empty affordances
 * array for unknown types (not 404).
 */
import { z } from "zod";
import { apiGet } from "./client";
import {
  affordanceSchema,
  type Affordance,
} from "./affordanceSchema";

/**
 * Fetch the affordance matrix for a given object type.
 * Returns 200 even for unknown types (empty array).
 */
export async function fetchAffordances(
  objectType: string,
): Promise<Affordance[]> {
  return apiGet(
    `/affordances/${encodeURIComponent(objectType)}`,
    z.array(affordanceSchema),
  );
}
