/**
 * Unknown block fallback renderer — renders raw JSON for unknown block ids.
 */
import type { UnknownViewBlock } from "../../../api/types";

export function UnknownBlockView({ block }: { block: UnknownViewBlock }) {
  return (
    <section
      data-testid="view-block-unknown"
      data-block-id={block.id}
      className="rounded-md p-3"
      style={{
        backgroundColor: "var(--color-surface-raised)",
        border: "1px dashed var(--color-warning)",
      }}
    >
      <header
        className="flex items-center justify-between gap-2"
        style={{ color: "var(--color-warning)" }}
      >
        <h3 className="text-xs font-semibold uppercase tracking-wide">
          {block.title}
        </h3>
        <span
          className="rounded-full px-2 py-0.5 font-mono text-xs"
          style={{
            backgroundColor: "var(--color-surface-overlay)",
            color: "var(--color-text-muted)",
          }}
        >
          unknown · {block.id}
        </span>
      </header>
      <p
        className="mt-1 text-xs"
        style={{ color: "var(--color-text-muted)" }}
      >
        This block kind is not yet rendered natively. Showing raw JSON.
      </p>
      <pre
        tabIndex={0}
        className="mt-2 overflow-x-auto rounded-sm p-2 font-mono text-xs"
        style={{
          backgroundColor: "var(--color-surface-overlay)",
          color: "var(--color-text-primary)",
        }}
      >
        <code data-testid="view-block-unknown-json">
          {JSON.stringify(block.body, null, 2)}
        </code>
      </pre>
    </section>
  );
}
