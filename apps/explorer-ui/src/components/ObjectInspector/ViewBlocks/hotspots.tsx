/**
 * Hotspots block renderer.
 */
import type { HotspotsBlockBody, ViewBlock } from "../../../api/types";
import { BlockShell } from "./shared";
import {
  type BlockRendererEntry,
  type BlockRendererProps,
  registerBlockRenderer,
} from "../blockRendererRegistry";

// Extra type for interactive blocks that need onSelectObject
type InteractiveExtra = { onSelectObject?: (id: string) => void };

function HotspotsViewAdapter({ block, extra }: BlockRendererProps<InteractiveExtra>) {
  return (
    <HotspotsView
      block={block as ViewBlock & { body: HotspotsBlockBody }}
      onSelectObject={extra?.onSelectObject}
    />
  );
}

registerBlockRenderer("hotspots", {
  component: HotspotsViewAdapter,
  displayName: "HotspotsView",
} as BlockRendererEntry);

export function HotspotsView({
  block,
  onSelectObject,
}: {
  block: ViewBlock & { body: HotspotsBlockBody };
  onSelectObject?: (id: string) => void;
}) {
  if (block.body.items.length === 0) {
    return (
      <BlockShell id={block.id} title={block.title}>
        <p style={{ color: "var(--color-text-muted)" }}>No hotspots.</p>
      </BlockShell>
    );
  }
  return (
    <BlockShell id={block.id} title={block.title}>
      <ol className="flex flex-col gap-0.5 text-sm">
        {block.body.items.map((it, idx) => (
          <li
            key={it.object_id}
            data-testid={`view-block-hotspot-${it.object_id}`}
            className="list-none"
          >
            {onSelectObject ? (
              <button
                type="button"
                onClick={() => onSelectObject(it.object_id)}
                data-testid={`view-block-hotspot-button-${it.object_id}`}
                className="flex w-full cursor-pointer items-center gap-2 rounded-sm px-2 py-1 text-left"
                style={{ backgroundColor: "var(--color-surface-overlay)" }}
              >
                <span
                  aria-hidden="true"
                  className="inline-flex h-5 w-5 flex-none items-center justify-center rounded-full font-mono text-xs"
                  style={{
                    backgroundColor: "var(--color-surface)",
                    color: "var(--color-text-muted)",
                  }}
                >
                  {idx + 1}
                </span>
                <span className="min-w-0 flex-1 truncate" title={it.name}>
                  {it.name}
                </span>
                <span
                  className="font-mono text-xs"
                  style={{ color: "var(--color-text-muted)" }}
                >
                  {it.file}:{it.line}
                </span>
              </button>
            ) : (
              <div
                className="flex items-center gap-2 rounded-sm px-2 py-1"
                style={{ backgroundColor: "var(--color-surface-overlay)" }}
              >
                <span
                  aria-hidden="true"
                  className="inline-flex h-5 w-5 flex-none items-center justify-center rounded-full font-mono text-xs"
                  style={{
                    backgroundColor: "var(--color-surface)",
                    color: "var(--color-text-muted)",
                  }}
                >
                  {idx + 1}
                </span>
                <span className="min-w-0 flex-1 truncate" title={it.name}>
                  {it.name}
                </span>
                <span
                  className="font-mono text-xs"
                  style={{ color: "var(--color-text-muted)" }}
                >
                  {it.file}:{it.line}
                </span>
              </div>
            )}
          </li>
        ))}
      </ol>
    </BlockShell>
  );
}
