/**
 * Entry point types for the LandingWorkbench "Start From" section.
 *
 * Each type maps to a Spotter pre-filter hint, so clicking it opens
 * Spotter with the kind chip already selected. This avoids re-implementing
 * entry-point resolution client-side.
 */
export type EntryPointKind = "route" | "use_case" | "symbol" | "event" | "saved_exploration";

export interface EntryPointType {
  id: EntryPointKind;
  label: string;
  description: string;
  icon: string;
  spotterKind: string;
}

export const ENTRY_POINT_TYPES: ReadonlyArray<EntryPointType> = [
  {
    id: "route",
    label: "Route",
    description: "HTTP endpoint or CLI command — start from an entry into the system.",
    icon: "↗",
    spotterKind: "route",
  },
  {
    id: "use_case",
    label: "Use case",
    description: "Business action — find what serves a specific intent.",
    icon: "◇",
    spotterKind: "use_case",
  },
  {
    id: "symbol",
    label: "Symbol",
    description: "Function, type, or module — drill into a specific code entity.",
    icon: "ƒ",
    spotterKind: "symbol",
  },
  {
    id: "event",
    label: "Event",
    description: "Domain event — follow what an event triggers downstream.",
    icon: "⚡",
    spotterKind: "event",
  },
  {
    id: "saved_exploration",
    label: "Saved exploration",
    description: "Resume a previous investigation from your recent work.",
    icon: "↺",
    spotterKind: "saved_exploration",
  },
];
