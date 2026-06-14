/**
 * ObjectInspector multimodal awareness — T19.
 *
 * The inspector must:
 * - Recognize the 4 multimodal `InspectableObjectType` extensions
 *   (`decision` / `doc` / `issue` / `evidence`) when they land
 *   on the wire. The legacy 9 types stay parseable unchanged.
 * - Surface contextual suggestions for multimodal nodes
 *   (e.g. "What code does this ADR justify?" for a Decision,
 *   "Where is this cited?" for a Doc, etc.).
 *
 * The recognition + suggestion surface lives in a small pure
 * helper so it can be unit-tested without rendering the full
 * `ObjectInspector` component (which requires the SWR cache +
 * the app-state context). The component itself reads the helper
 * in `index.tsx`.
 */
import {
  type GraphNodeStyleClass,
  type InspectableObjectType,
} from "../../api/types";

/**
 * The 4 multimodal node kinds we want to recognise, paired with
 * the corresponding wire `style_class` and a label used by the
 * header badge.
 */
export interface MultimodalKindInfo {
  /** Wire `style_class` value (matches the cytoscape selector). */
  readonly styleClass: Exclude<GraphNodeStyleClass, "function" | "module" | "external">;
  /** Stable label rendered on the header badge. */
  readonly badgeLabel: string;
  /** Tailwind-style palette token (background + foreground). */
  readonly palette: { readonly background: string; readonly foreground: string };
  /** 3-5 follow-up prompts a user can fire against the object. */
  readonly suggestions: readonly MultimodalSuggestion[];
}

export interface MultimodalSuggestion {
  readonly id: string;
  readonly label: string;
  readonly question: string;
}

/**
 * The full map of multimodal kinds to their `MultimodalKindInfo`.
 * The 3 legacy kinds (function / module / external) are NOT
 * multimodal — they are absent here. Callers MUST check the
 * return of [`recognizeMultimodalKind`] before accessing the map.
 */
export const MULTIMODAL_KIND_INFO: Readonly<
  Record<Exclude<GraphNodeStyleClass, "function" | "module" | "external">, MultimodalKindInfo>
> = {
  "node-decision": {
    styleClass: "node-decision",
    badgeLabel: "Decision",
    palette: { background: "#f59e0b", foreground: "#7c2d12" },
    suggestions: [
      { id: "dec-justifies", label: "What does this justify?", question: "what does this decision justify?" },
      { id: "dec-cited-by", label: "Where is this cited?", question: "where is this decision cited?" },
      { id: "dec-supersedes", label: "Does this supersede another ADR?", question: "does this decision supersede another ADR?" },
    ],
  },
  "node-doc": {
    styleClass: "node-doc",
    badgeLabel: "Doc",
    palette: { background: "#14b8a6", foreground: "#134e4a" },
    suggestions: [
      { id: "doc-cites", label: "What does this cite?", question: "what symbols does this doc cite?" },
      { id: "doc-cited-by", label: "Where is this cited?", question: "where is this doc cited?" },
      { id: "doc-section", label: "Show section headings", question: "show the section headings of this doc" },
    ],
  },
  "node-issue": {
    styleClass: "node-issue",
    badgeLabel: "Issue",
    palette: { background: "#ef4444", foreground: "#7f1d1d" },
    suggestions: [
      { id: "iss-resolves", label: "What does this resolve?", question: "what does this issue resolve?" },
      { id: "iss-resolved-by", label: "What resolved this?", question: "what PR / commit resolved this issue?" },
      { id: "iss-related", label: "Related issues", question: "what issues are related to this one?" },
    ],
  },
  "node-evidence": {
    styleClass: "node-evidence",
    badgeLabel: "Evidence",
    palette: { background: "#a855f7", foreground: "#581c87" },
    suggestions: [
      { id: "ev-corroborates", label: "What does this corroborate?", question: "what claim does this evidence corroborate?" },
      { id: "ev-corroborated-by", label: "Other corroborating evidence", question: "what other evidence corroborates the same claim?" },
      { id: "ev-freshness", label: "How fresh is this?", question: "how fresh is this evidence?" },
    ],
  },
  "node-component": {
    styleClass: "node-component",
    badgeLabel: "Component",
    palette: { background: "#3b82f6", foreground: "#1e3a8a" },
    suggestions: [
      { id: "comp-owns", label: "What does this own?", question: "what does this component own?" },
      { id: "comp-deployed-as", label: "How is this deployed?", question: "how is this component deployed?" },
    ],
  },
  "node-container": {
    styleClass: "node-container",
    badgeLabel: "Container",
    palette: { background: "#06b6d4", foreground: "#164e63" },
    suggestions: [
      { id: "cont-contains", label: "What does this contain?", question: "what components does this container contain?" },
      { id: "cont-deployed-as", label: "How is this deployed?", question: "how is this container deployed?" },
    ],
  },
  "node-system": {
    styleClass: "node-system",
    badgeLabel: "System",
    palette: { background: "#10b981", foreground: "#064e3b" },
    suggestions: [
      { id: "sys-contains", label: "What does this contain?", question: "what containers does this system contain?" },
      { id: "sys-context", label: "System context", question: "what is the context of this system?" },
    ],
  },
};

/**
 * Resolve a `GraphNodeStyleClass` to its multimodal info, or
 * `null` if the kind is a code-only `function` / `module` /
 * `external` (no multimodal awareness needed).
 */
export function recognizeMultimodalKind(
  styleClass: string | null | undefined,
): MultimodalKindInfo | null {
  if (!styleClass) return null;
  if (styleClass in MULTIMODAL_KIND_INFO) {
    return MULTIMODAL_KIND_INFO[
      styleClass as Exclude<GraphNodeStyleClass, "function" | "module" | "external">
    ];
  }
  return null;
}

/**
 * Map an `InspectableObjectType` (the legacy 9-variant enum)
 * to a multimodal header label. Returns `null` when the object
 * type is a code-only kind — the inspector should NOT show a
 * multimodal badge in that case.
 */
export function multimodalLabelForObjectType(
  type: InspectableObjectType,
): string | null {
  switch (type) {
    case "decision_artifact":
      return "Decision";
    case "evidence":
      return "Evidence";
    default:
      return null;
  }
}
