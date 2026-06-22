/**
 * File-related block renderers: FileIdentityView, FileSymbolsView, KindsView.
 */
import type {
  FileIdentityBlockBody,
  FileSymbolsBlockBody,
  KindsBreakdownBlockBody,
  ViewBlock,
} from "../../../api/types";
import { BlockShell, Stat } from "./shared";
import {
  type BlockRendererEntry,
  type BlockRendererProps,
  registerBlockRenderer,
} from "../blockRendererRegistry";

// ============================================================================
// FileIdentityView
// ============================================================================

export function FileIdentityView({
  block,
}: {
  block: ViewBlock & { body: FileIdentityBlockBody };
}) {
  const b = block.body;
  return (
    <BlockShell id={block.id} title={block.title}>
      <p
        className="font-mono text-xs"
        style={{ color: "var(--color-text-secondary)" }}
      >
        {b.path}
      </p>
      <dl className="mt-1 grid grid-cols-2 gap-1 text-xs">
        <Stat label="Lines" value={b.line_count} small />
        <Stat label="Symbols" value={b.symbol_count} small />
      </dl>
    </BlockShell>
  );
}

// ============================================================================
// KindsView
// ============================================================================

export function KindsView({
  block,
}: {
  block: ViewBlock & { body: KindsBreakdownBlockBody };
}) {
  const entries = Object.entries(block.body.breakdown);
  if (entries.length === 0) {
    return (
      <BlockShell id={block.id} title={block.title}>
        <p style={{ color: "var(--color-text-muted)" }}>No symbols.</p>
      </BlockShell>
    );
  }
  return (
    <BlockShell id={block.id} title={block.title}>
      <dl className="grid grid-cols-2 gap-1 text-xs">
        {entries.map(([kind, count]) => (
          <div
            key={kind}
            className="flex items-center justify-between rounded-sm px-2 py-1"
            style={{ backgroundColor: "var(--color-surface-overlay)" }}
          >
            <dt style={{ color: "var(--color-text-secondary)" }}>{kind}</dt>
            <dd className="font-mono">{count}</dd>
          </div>
        ))}
      </dl>
    </BlockShell>
  );
}

// ============================================================================
// FileSymbolsView
// ============================================================================

export function FileSymbolsView({
  block,
}: {
  block: ViewBlock & { body: FileSymbolsBlockBody };
}) {
  if (block.body.items.length === 0) {
    return (
      <BlockShell id={block.id} title={block.title}>
        <p style={{ color: "var(--color-text-muted)" }}>No symbols.</p>
      </BlockShell>
    );
  }
  return (
    <BlockShell id={block.id} title={block.title}>
      <ul className="flex flex-col gap-0.5 text-sm">
        {block.body.items.map((it) => (
          <li
            key={it.object_id}
            className="flex items-center gap-2 rounded-sm px-2 py-1"
            style={{ backgroundColor: "var(--color-surface-overlay)" }}
          >
            <span
              aria-hidden="true"
              className="font-mono text-xs"
              style={{ color: "var(--color-text-muted)" }}
            >
              ƒ
            </span>
            <span className="min-w-0 flex-1 truncate" title={it.name}>
              {it.name}
            </span>
            <span
              className="font-mono text-xs"
              style={{ color: "var(--color-text-muted)" }}
            >
              {it.kind} · {it.line}
            </span>
          </li>
        ))}
      </ul>
    </BlockShell>
  );
}

// ============================================================================
// Registry adapters
// ============================================================================

function FileIdentityViewAdapter({ block }: BlockRendererProps) {
  return (
    <FileIdentityView
      block={block as ViewBlock & { body: FileIdentityBlockBody }}
    />
  );
}

function FileSymbolsViewAdapter({ block }: BlockRendererProps) {
  return (
    <FileSymbolsView
      block={block as ViewBlock & { body: FileSymbolsBlockBody }}
    />
  );
}

function KindsViewAdapter({ block }: BlockRendererProps) {
  return (
    <KindsView
      block={block as ViewBlock & { body: KindsBreakdownBlockBody }}
    />
  );
}

registerBlockRenderer("file_identity", {
  component: FileIdentityViewAdapter,
  displayName: "FileIdentityView",
} as BlockRendererEntry);

registerBlockRenderer("symbols", {
  component: FileSymbolsViewAdapter,
  displayName: "FileSymbolsView",
} as BlockRendererEntry);

registerBlockRenderer("kinds", {
  component: KindsViewAdapter,
  displayName: "KindsView",
} as BlockRendererEntry);

registerBlockRenderer("scope_kinds", {
  component: KindsViewAdapter,
  displayName: "KindsView",
} as BlockRendererEntry);
