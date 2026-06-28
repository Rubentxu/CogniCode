/**
 * InvestigationsSection — common investigation starters for the workbench.
 *
 * Lists reusable investigation intents. Each one opens Spotter with
 * a pre-filled query (the existing useSpotter debounce handles the
 * pre-fill naturally because Spotter re-fetches when rawQuery
 * changes — so we set rawQuery via a custom event).
 *
 * In v1 we keep it simple: clicking opens Spotter empty; the user
 * types their own query. Future v2 will pre-fill via a shared event.
 */
import { useAppDispatch } from "../../state/context";

interface InvestigationTemplate {
  id: string;
  label: string;
  description: string;
  intent: string; // TODO v2: pre-fill Spotter
}

const INVESTIGATION_TEMPLATES: ReadonlyArray<InvestigationTemplate> = [
  {
    id: "trace-request",
    label: "Trace a request",
    description: "Follow an HTTP route through handlers, use cases, and persistence.",
    intent: "trace request",
  },
  {
    id: "impact-radius",
    label: "Find impact radius",
    description: "See who calls a symbol, what depends on it, and what tests cover it.",
    intent: "impact",
  },
  {
    id: "understand-ownership",
    label: "Understand ownership",
    description: "Map modules, components, and decisions to their owning concerns.",
    intent: "ownership",
  },
  {
    id: "review-change-impact",
    label: "Review change impact",
    description: "Trace upstream callers and downstream callees before refactoring.",
    intent: "change impact",
  },
  {
    id: "explain-decision",
    label: "Explain a decision",
    description: "Link an ADR or doc to the code that implements it.",
    intent: "decision",
  },
];

export function InvestigationsSection() {
  const dispatch = useAppDispatch();

  const handleClick = (tpl: InvestigationTemplate) => {
    // v1: just open Spotter. v2 will pre-fill rawQuery via a shared event.
    dispatch({
      type: "SET_SPOTTER",
      payload: { open: true },
    });
    // Hint stored for future pre-fill (read by Spotter v2)
    window.dispatchEvent(
      new CustomEvent("cognicode:investigation-hint", { detail: tpl.intent }),
    );
  };

  return (
    <div
      data-testid="investigations-section"
      className="flex flex-col gap-4 p-6"
      aria-label="Common investigations"
    >
      <header>
        <h2
          className="text-sm font-semibold"
          style={{ color: "var(--color-text-primary)" }}
        >
          Common investigations
        </h2>
        <p
          className="mt-1 text-xs"
          style={{ color: "var(--color-text-muted)" }}
        >
          Reusable starting points. Each one opens Spotter with a hint.
        </p>
      </header>
      <ul className="flex flex-col gap-2">
        {INVESTIGATION_TEMPLATES.map((tpl) => (
          <li key={tpl.id}>
            <button
              type="button"
              data-testid={`investigation-template-${tpl.id}`}
              onClick={() => handleClick(tpl)}
              className="w-full rounded-md border px-4 py-3 text-left transition-colors"
              style={{
                borderColor: "var(--color-border)",
                backgroundColor: "var(--color-surface-raised)",
              }}
            >
              <div
                className="text-sm font-medium"
                style={{ color: "var(--color-text-primary)" }}
              >
                {tpl.label}
              </div>
              <p
                className="mt-1 text-xs"
                style={{ color: "var(--color-text-muted)" }}
              >
                {tpl.description}
              </p>
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
