/**
 * `ViewIcon` — small icon set for the ViewTabs strip.
 *
 * Each view gets a unique 16×16 SVG icon. Icons use `currentColor` so they
 * inherit the tab's active/inactive color state without needing two
 * variants per icon.
 *
 * Naming: each entry maps a view id to a glyph. New views added to the
 * registry without an icon fall back to `<ViewIconFallback>` (a small dot).
 */
import type { JSX } from "react";

const COMMON_PROPS = {
  width: 14,
  height: 14,
  viewBox: "0 0 16 16",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.5,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
  "aria-hidden": true,
};

export const VIEW_ICONS: Record<string, (props: { className?: string }) => JSX.Element> = {
  // Overview — dashboard panels
  overview: () => (
    <svg {...COMMON_PROPS}>
      <rect x="2" y="2" width="5" height="5" rx="1" />
      <rect x="9" y="2" width="5" height="3" rx="1" />
      <rect x="9" y="7" width="5" height="7" rx="1" />
      <rect x="2" y="9" width="5" height="5" rx="1" />
    </svg>
  ),
  // Call graph — node with edges
  "call-graph": () => (
    <svg {...COMMON_PROPS}>
      <circle cx="4" cy="4" r="2" />
      <circle cx="12" cy="4" r="2" />
      <circle cx="8" cy="12" r="2" />
      <path d="M5.5 5.5 7 10.5" />
      <path d="M10.5 5.5 9 10.5" />
      <path d="M6 4 H10" />
    </svg>
  ),
  // Source — code brackets
  source: () => (
    <svg {...COMMON_PROPS}>
      <path d="M5 4 2 8 5 12" />
      <path d="M11 4 14 8 11 12" />
      <path d="M9 3 7 13" />
    </svg>
  ),
  // Quality — checkmark in shield
  quality: () => (
    <svg {...COMMON_PROPS}>
      <path d="M8 1.5 13 3.5 V8 C13 11 11 13 8 14.5 5 13 3 11 3 8 V3.5 Z" />
      <path d="M6 8 7.5 9.5 10.5 6.5" />
    </svg>
  ),
  // Evidence — pin
  evidence: () => (
    <svg {...COMMON_PROPS}>
      <path d="M8 1.5 V6.5" />
      <path d="M5 4.5 H11" />
      <path d="M8 6.5 V14.5" />
      <circle cx="8" cy="3.5" r="1" />
    </svg>
  ),
  // Symbols — nested shapes
  symbols: () => (
    <svg {...COMMON_PROPS}>
      <circle cx="5" cy="6" r="2" />
      <rect x="9" y="4" width="4" height="4" rx="0.5" />
      <path d="M3 12 L7 9 L11 11 L13 9" />
    </svg>
  ),
  // Dependencies — connected nodes
  dependencies: () => (
    <svg {...COMMON_PROPS}>
      <circle cx="3" cy="8" r="1.5" />
      <circle cx="8" cy="3" r="1.5" />
      <circle cx="8" cy="13" r="1.5" />
      <circle cx="13" cy="8" r="1.5" />
      <path d="M4.5 8 H6.5" />
      <path d="M8 4.5 V6.5" />
      <path d="M8 9.5 V11.5" />
      <path d="M9.5 8 H11.5" />
    </svg>
  ),
  // Hotspots — flame
  hotspots: () => (
    <svg {...COMMON_PROPS}>
      <path d="M8 14.5 C5 14.5 3 12.5 3 9.5 C3 7 5 5 6 3 C6 5 7 6 8 6 C9 6 10 4 10 2 C11 4 13 6 13 9 C13 12 11 14.5 8 14.5 Z" />
    </svg>
  ),
  // Architecture drift — split arrow
  "architecture-drift": () => (
    <svg {...COMMON_PROPS}>
      <path d="M2 8 L7 8" />
      <path d="M5 6 L7 8 L5 10" />
      <path d="M14 8 L9 8" />
      <path d="M11 6 L9 8 L11 10" />
    </svg>
  ),
  // Usage examples — magnifying glass
  "usage-examples": () => (
    <svg {...COMMON_PROPS}>
      <circle cx="7" cy="7" r="4.5" />
      <path d="M10.5 10.5 L13.5 13.5" />
    </svg>
  ),
  // API surface — cube
  "api-surface": () => (
    <svg {...COMMON_PROPS}>
      <path d="M8 1.5 L13.5 4.5 V11 L8 14 L2.5 11 V4.5 Z" />
      <path d="M2.5 4.5 L8 7.5 L13.5 4.5" />
      <path d="M8 7.5 V14" />
    </svg>
  ),
  // Test slice — filter funnel
  "test-slice": () => (
    <svg {...COMMON_PROPS}>
      <path d="M2 2 H14 L9 8 V14 L7 13 V8 Z" />
    </svg>
  ),
  // Debug slice — bug
  "debug-slice": () => (
    <svg {...COMMON_PROPS}>
      <ellipse cx="8" cy="9" rx="5" ry="5" />
      <path d="M5 6 L3 4" />
      <path d="M11 6 L13 4" />
      <path d="M6 4 L6 2" />
      <path d="M10 4 L10 2" />
      <circle cx="8" cy="9" r="0.5" fill="currentColor" />
    </svg>
  ),
  // Change impact story — pulse
  "change-impact-story": () => (
    <svg {...COMMON_PROPS}>
      <circle cx="8" cy="8" r="2" />
      <path d="M8 4 V5.5" />
      <path d="M8 10.5 V12" />
      <path d="M4 8 H5.5" />
      <path d="M10.5 8 H12" />
      <path d="M5 5 L6 6" />
      <path d="M10 10 L11 11" />
      <path d="M11 5 L10 6" />
      <path d="M6 10 L5 11" />
    </svg>
  ),
  // Ownership map — user circle
  "ownership-map": () => (
    <svg {...COMMON_PROPS}>
      <circle cx="8" cy="5" r="2.5" />
      <path d="M3 14 C3 11 5 9 8 9 C11 9 13 11 13 14" />
    </svg>
  ),
};

export function ViewIcon({ id, className }: { id: string; className?: string }): JSX.Element {
  const Icon = VIEW_ICONS[id];
  if (Icon) return <Icon />;
  // Fallback for unknown view ids — small dot
  return (
    <svg
      width={14}
      height={14}
      viewBox="0 0 16 16"
      aria-hidden="true"
      className={className}
      style={{ fill: "currentColor", stroke: "none" }}
    >
      <circle cx="8" cy="8" r="2" />
    </svg>
  );
}
