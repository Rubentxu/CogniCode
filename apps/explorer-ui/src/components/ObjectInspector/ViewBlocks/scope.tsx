/**
 * Scope-related block renderers: ScopeIdentityView, ScopeFilesView, CrossScopeView.
 */
import type {
  CrossScopeBlockBody,
  ScopeFilesBlockBody,
  ScopeIdentityBlockBody,
  ViewBlock,
} from "../../../api/types";
import { BlockShell, Stat } from "./shared";

// ============================================================================
// ScopeIdentityView
// ============================================================================

export function ScopeIdentityView({
  block,
}: {
  block: ViewBlock & { body: ScopeIdentityBlockBody };
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
        <Stat label="Files" value={b.file_count} small />
        <Stat label="Symbols" value={b.symbol_count} small />
        <div
          className="col-span-2 flex items-center justify-between rounded-sm px-2 py-1"
          style={{ backgroundColor: "var(--color-surface-overlay)" }}
        >
          <dt style={{ color: "var(--color-text-secondary)" }}>
            Promotion ready
          </dt>
          <dd
            className="font-mono"
            style={{
              color: b.promotion_ready
                ? "var(--color-success)"
                : "var(--color-text-muted)",
            }}
          >
            {b.promotion_ready ? "yes" : "no"}
          </dd>
        </div>
      </dl>
    </BlockShell>
  );
}

// ============================================================================
// ScopeFilesView
// ============================================================================

export function ScopeFilesView({
  block,
}: {
  block: ViewBlock & { body: ScopeFilesBlockBody };
}) {
  if (block.body.files.length === 0) {
    return (
      <BlockShell id={block.id} title={block.title}>
        <p style={{ color: "var(--color-text-muted)" }}>No files.</p>
      </BlockShell>
    );
  }
  return (
    <BlockShell id={block.id} title={block.title}>
      <ul className="flex flex-col gap-0.5 text-xs">
        {block.body.files.map((f) => (
          <li
            key={f}
            className="rounded-sm px-2 py-1 font-mono"
            style={{ backgroundColor: "var(--color-surface-overlay)" }}
          >
            {f}
          </li>
        ))}
      </ul>
    </BlockShell>
  );
}

// ============================================================================
// CrossScopeView
// ============================================================================

export function CrossScopeView({
  block,
}: {
  block: ViewBlock & { body: CrossScopeBlockBody };
}) {
  if (block.body.entries.length === 0) {
    return (
      <BlockShell id={block.id} title={block.title}>
        <p style={{ color: "var(--color-text-muted)" }}>
          No cross-scope relations.
        </p>
      </BlockShell>
    );
  }
  return (
    <BlockShell id={block.id} title={block.title}>
      <p
        className="font-mono text-xs"
        style={{ color: "var(--color-text-muted)" }}
      >
        {block.body.scope}
      </p>
      <table className="mt-2 w-full text-xs">
        <thead style={{ color: "var(--color-text-muted)" }}>
          <tr className="text-left">
            <th className="px-2 py-1 font-medium">Scope</th>
            <th className="px-2 py-1 font-medium">Out</th>
            <th className="px-2 py-1 font-medium">In</th>
          </tr>
        </thead>
        <tbody>
          {block.body.entries.map((e) => (
            <tr
              key={e.scope}
              style={{ borderTop: "1px solid var(--color-border)" }}
            >
              <td
                className="px-2 py-1 font-mono"
                style={{ color: "var(--color-text-primary)" }}
              >
                {e.scope}
              </td>
              <td className="px-2 py-1 font-mono">{e.outgoing_count}</td>
              <td className="px-2 py-1 font-mono">{e.incoming_count}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </BlockShell>
  );
}
