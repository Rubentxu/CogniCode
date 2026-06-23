/**
 * `ViewBlock` — render a single block from a `ContextualView`.
 *
 * Dispatch is via `blockRendererRegistry` — a self-registering Map from
 * block id to renderer entry. Each block component registers itself at
 * module load; this file only does the lookup.
 *
 * Exhaustiveness is enforced at registration time (assert all KNOWN_IDS
 * are registered) and verified by `blockRendererRegistry.test.ts`.
 *
 * The `Blocks` wrapper component is unchanged — it still iterates blocks
 * and calls `ViewBlock` per item.
 */
import { useMemo } from "react";

import type {
  ContextualView,
  ViewBlock,
  UnknownViewBlock,
} from "../../api/types";

import {
  blockRendererRegistry,
  UnknownBlockView,
} from "./blockRendererRegistry";

// ============================================================================
// Side-effect imports — trigger block registrations at module load
// These MUST come AFTER the blockRendererRegistry import so that
// registerBlockRenderer is available when they execute.
// ============================================================================
import "./ViewBlocks/call";
import "./ViewBlocks/file";
import "./ViewBlocks/hotspots";
import "./ViewBlocks/identity";
import "./ViewBlocks/quality";
import "./ViewBlocks/scope";

// ============================================================================
// Extra context type (subset of RuntimeContext — only what blocks need)
// ============================================================================

/**
 * The subset of RuntimeContext that block renderers care about.
 * Interactive blocks (callers, callees, hotspots, quality_issue_detail)
 * use `onSelectObject`. The rest ignore extra entirely.
 */
export interface BlockExtra {
  onSelectObject?: (objectId: string) => void;
}

// ============================================================================
// Public component
// ============================================================================

export interface ViewBlockProps {
  /** The block to render. We accept `ViewBlockAny` so unknown blocks fall through. */
  block: ViewBlock | UnknownViewBlock;
  /**
   * Optional callback when the user picks a related object (a
   * caller / callee / hotspot etc). When present, those items
   * become interactive.
   *
   * Passed through to the registry entry via `extra.onSelectObject`.
   */
  onSelectObject?: (objectId: string) => void;
}

/**
 * Route a block to its registered renderer.
 *
 * If the block id has a registered renderer, it is rendered with
 * `onSelectObject` forwarded via `extra`. Otherwise the unknown-block
 * fallback is shown.
 */
export function ViewBlock({ block, onSelectObject }: ViewBlockProps) {
  const id = (block as { id: string }).id;
  const entry = blockRendererRegistry.get(id);

  if (!entry) {
    return <UnknownBlockView block={block as UnknownViewBlock} />;
  }

  return (
    <entry.component
      block={block as ViewBlock}
      objectId=""
      extra={onSelectObject ? { onSelectObject } : undefined}
    />
  );
}

// ============================================================================
// Blocks — render a list of blocks in a vertical stack
// ============================================================================

export interface BlocksProps {
  view: ContextualView;
  /**
   * Optional navigation callback. When provided, interactive block items
   * become clickable.
   *
   * E1.5 (PaneInspector): passed via `extra.onSelectObject` from runtimeContext.
   * Legacy / tests: passed directly as the `onSelectObject` prop.
   */
  onSelectObject?: (objectId: string) => void;
}

/**
 * Render all blocks in a `ContextualView`. This is the convenience
 * wrapper used by the Inspector container — it iterates the
 * discriminated union and hands each block to `ViewBlock`.
 */
export function Blocks({ view, onSelectObject }: BlocksProps) {
  const items = useMemo(() => view.blocks, [view.blocks]);
  if (items.length === 0) {
    return (
      <div
        data-testid="view-blocks-empty"
        className="p-4 text-sm"
        style={{ color: "var(--color-text-muted)" }}
      >
        This view has no blocks yet.
      </div>
    );
  }
  return (
    <div
      data-testid="view-blocks"
      className="flex flex-col gap-2"
    >
      {items.map((block, idx) => (
        <ViewBlock
          key={`${(block as { id: string }).id}-${idx}`}
          block={block}
          onSelectObject={onSelectObject}
        />
      ))}
    </div>
  );
}
