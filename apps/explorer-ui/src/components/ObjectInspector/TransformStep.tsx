/**
 * `TransformStep` — Step 4 of the ViewSpecWizard.
 *
 * Lets users choose a JSONata transform and provides a live side-by-side
 * preview of input → output as they type.
 *
 * Auto-triggers the preview 300ms after the last keystroke (debounced
 * via `useJsonataPreview`). The preview panel shows:
 *   - Left: raw MoldQL result (the `input` prop)
 *   - Right: transformed result (from the JSONata worker)
 *   - Bottom: inline error in red when evaluation fails
 *
 * Lazy-loads the JSONata worker — no impact on initial bundle size.
 */
import { useJsonataPreview } from "../../hooks/useJsonataPreview";

export interface TransformStepProps {
  transformKind: "none" | "jsonata";
  expression: string;
  /** The MoldQL data source output used as JSONata input. */
  previewInput: unknown;
  onTransformKindChange: (tk: "none" | "jsonata") => void;
  onExpressionChange: (e: string) => void;
}

export function TransformStep({
  transformKind,
  expression,
  previewInput,
  onTransformKindChange,
  onExpressionChange,
}: TransformStepProps) {
  const { output, error, loading } = useJsonataPreview(
    previewInput,
    transformKind === "jsonata" && expression.trim().length > 0 ? expression : null,
  );

  return (
    <div className="flex flex-col gap-4">
      <div>
        <h3 className="mb-1 text-sm font-medium" style={{ color: "var(--color-text-primary)" }}>
          Transform (Optional)
        </h3>
        <p className="text-xs" style={{ color: "var(--color-text-muted)" }}>
          Reshape the MoldQL result before rendering. JSONata is a lightweight JSON query
          language. Leave as "No transform" for raw MoldQL output.
        </p>
      </div>

      {/* Transform kind toggle */}
      <div className="flex gap-2">
        <button
          type="button"
          onClick={() => onTransformKindChange("none")}
          className="rounded-md px-4 py-2 text-sm font-medium transition-colors"
          style={{
            backgroundColor: transformKind === "none" ? "var(--color-primary)" : "var(--color-surface-overlay)",
            color: transformKind === "none" ? "var(--color-primary-foreground)" : "var(--color-text-primary)",
          }}
        >
          No transform
        </button>
        <button
          type="button"
          onClick={() => onTransformKindChange("jsonata")}
          className="rounded-md px-4 py-2 text-sm font-medium transition-colors"
          style={{
            backgroundColor: transformKind === "jsonata" ? "var(--color-primary)" : "var(--color-surface-overlay)",
            color: transformKind === "jsonata" ? "var(--color-primary-foreground)" : "var(--color-text-primary)",
          }}
        >
          JSONata
        </button>
      </div>

      {/* JSONata editor + preview */}
      {transformKind === "jsonata" && (
        <div className="flex flex-col gap-2">
          <textarea
            value={expression}
            onChange={(e) => onExpressionChange(e.target.value)}
            placeholder="items[fan_out > 5]"
            rows={4}
            className="w-full rounded-md px-3 py-2 font-mono text-xs"
            style={{
              backgroundColor: "var(--color-surface-overlay)",
              color: "var(--color-text-primary)",
              border: "1px solid var(--color-border)",
              resize: "vertical",
            }}
          />
          <div
            className="rounded-md p-3 text-xs"
            style={{ backgroundColor: "var(--color-surface-overlay)", color: "var(--color-text-muted)" }}
          >
            <strong>JSONata examples:</strong>
            <ul className="mt-1 list-disc pl-4">
              <li>
                <code className="font-mono">items[fan_out &gt; 5]</code> — filter to high fan-out
              </li>
              <li>
                <code className="font-mono">items.orderBy($fan_out, 'desc')</code> — sort by fan_out desc
              </li>
              <li>
                <code className="font-mono">$count(items)</code> — count items
              </li>
            </ul>
          </div>

          {/* Side-by-side preview */}
          <div
            className="flex flex-col gap-2 rounded-md border p-3"
            style={{ borderColor: "var(--color-border)" }}
          >
            <div className="flex items-center justify-between">
              <span className="text-xs font-semibold" style={{ color: "var(--color-text-secondary)" }}>
                Live Preview
              </span>
              {loading && (
                <span className="text-xs" style={{ color: "var(--color-text-muted)" }}>
                  Evaluating…
                </span>
              )}
            </div>

            {/* Input / Output side-by-side */}
            <div className="grid grid-cols-2 gap-2">
              <div>
                <p className="mb-1 text-[10px] font-semibold uppercase tracking-wide" style={{ color: "var(--color-text-muted)" }}>
                  Input
                </p>
                <pre
                  className="max-h-32 overflow-auto rounded p-2 text-[10px] font-mono"
                  style={{
                    backgroundColor: "var(--color-surface)",
                    color: "var(--color-text-secondary)",
                    border: "1px solid var(--color-border)",
                  }}
                >
                  {JSON.stringify(previewInput, null, 2).slice(0, 500)}
                  {JSON.stringify(previewInput).length > 500 ? "\n…(truncated)" : ""}
                </pre>
              </div>
              <div>
                <p className="mb-1 text-[10px] font-semibold uppercase tracking-wide" style={{ color: "var(--color-text-muted)" }}>
                  Output
                </p>
                <pre
                  className="max-h-32 overflow-auto rounded p-2 text-[10px] font-mono"
                  style={{
                    backgroundColor: "var(--color-surface)",
                    color: error ? "var(--color-error)" : "var(--color-text-primary)",
                    border: `1px solid ${error ? "var(--color-error)" : "var(--color-border)"}`,
                  }}
                >
                  {error
                    ? `Error: ${error}`
                    : output !== null
                      ? JSON.stringify(output, null, 2).slice(0, 500)
                      : "Enter an expression above to see output."}
                </pre>
              </div>
            </div>

            {/* Inline error below editor */}
            {error && (
              <div
                className="rounded-md p-2 text-xs"
                style={{
                  backgroundColor: "rgba(239,68,68,0.1)",
                  color: "var(--color-error)",
                  border: "1px solid var(--color-error)",
                }}
              >
                {error}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
