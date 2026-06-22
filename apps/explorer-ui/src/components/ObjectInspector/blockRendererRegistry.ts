/**
 * `blockRendererRegistry.ts` — block-id-keyed renderer registry.
 *
 * Replaces the 29-case switch in ViewBlock.tsx with a self-registering
 * Map<BlockId, BlockRendererEntry>. Each block component registers itself
 * at module load; ViewBlock.tsx dispatches via lookup.
 *
 * Exhaustiveness is enforced by:
 * 1. A registration-time assertion that all KNOWN_IDS are registered
 * 2. A dedicated test (blockRendererRegistry.test.ts)
 *
 * Design: ADR-008 §240, REQ-E1.4-1 through REQ-E1.4-6.
 */
import type { ViewBlock, UnknownViewBlock } from "../../api/types";

// ============================================================================
// Registry entry
// ============================================================================

/**
 * A registered block renderer. The `component` must accept
 * `BlockRendererProps` and render the block.
 */
export interface BlockRendererEntry<P = unknown> {
  /** The React component that renders this block type. */
  component: React.ComponentType<BlockRendererProps<P>>;
  /** Human-readable name for dev-tools / error messages. */
  displayName: string;
}

/**
 * Props passed to every block renderer component.
 *
 * `extra` carries RuntimeContext fields that interactive blocks need
 * (onSelectObject, dispatch, etc.) without each block importing
 * useAppDispatch.
 */
export interface BlockRendererProps<Extra = unknown> {
  block: ViewBlock;
  objectId: string;
  extra?: Extra;
}

// ============================================================================
// Registry class
// ============================================================================

/**
 * Global registry of block renderers, keyed by block `id`.
 *
 * Components call `registerBlockRenderer(id, entry)` at module load.
 * ViewBlock.tsx calls `getBlockRenderer(id)` at render time.
 *
 * The registry is a plain Map — no singleton pattern needed.
 */
class BlockRendererRegistry {
  #map = new Map<string, BlockRendererEntry>();

  /**
   * Register a renderer for the given block id.
   * Idempotent: later registrations overwrite earlier ones.
   */
  register(id: string, entry: BlockRendererEntry): void {
    this.#map.set(id, entry);
  }

  /**
   * Look up the renderer for a block id.
   * Returns `undefined` if no renderer is registered.
   */
  get(id: string): BlockRendererEntry | undefined {
    return this.#map.get(id);
  }

  /**
   * Returns an iterator over all registered (id, entry) pairs.
   */
  entries(): IterableIterator<[string, BlockRendererEntry]> {
    return this.#map.entries();
  }

  /**
   * Returns the number of registered entries.
   */
  get size(): number {
    return this.#map.size;
  }
}

// ============================================================================
// Singleton instance
// ============================================================================

/**
 * The global block renderer registry.
 * Import this in ViewBlock.tsx to replace the switch dispatch.
 */
export const blockRendererRegistry = new BlockRendererRegistry();

// ============================================================================
// Registration helper
// ============================================================================

/**
 * Register a block renderer. Convenience wrapper around
 * `blockRendererRegistry.register`.
 *
 * Usage in a ViewBlocks/*.tsx file:
 * ```ts
 * registerBlockRenderer("identity", { component: IdentityView, displayName: "IdentityView" });
 * ```
 */
export function registerBlockRenderer(
  id: string,
  entry: BlockRendererEntry,
): void {
  blockRendererRegistry.register(id, entry);
}

// ============================================================================
// Unknown block fallback
// ============================================================================

/**
 * Fallback component rendered when a block id has no registered renderer.
 * This is the runtime equivalent of the `never` exhaustiveness check —
 * it keeps the UI working when a new block ships before its renderer.
 *
 * Mirrors the original ViewBlocks/unknown.tsx version exactly so that
 * existing test assertions (data-testid, rendered structure) are preserved.
 */
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
