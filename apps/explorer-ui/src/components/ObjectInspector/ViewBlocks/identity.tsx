/**
 * Identity block renderer — shows symbol name, kind, and file location.
 */
import type { IdentityBlockBody, ViewBlock } from "../../../api/types";
import { BlockShell } from "./shared";
import {
  type BlockRendererEntry,
  type BlockRendererProps,
  registerBlockRenderer,
} from "../blockRendererRegistry";

export function IdentityView({ block }: { block: ViewBlock & { body: IdentityBlockBody } }) {
  const b = block.body;
  return (
    <BlockShell id={block.id} title={block.title}>
      <p>
        <span className="font-semibold">{b.name}</span>{" "}
        <span style={{ color: "var(--color-text-muted)" }}>· {b.kind}</span>
      </p>
      <p
        className="font-mono text-xs"
        style={{ color: "var(--color-text-secondary)" }}
      >
        {b.file}:{b.line}
      </p>
    </BlockShell>
  );
}

function IdentityViewAdapter({ block }: BlockRendererProps) {
  return <IdentityView block={block as ViewBlock & { body: IdentityBlockBody }} />;
}

registerBlockRenderer("identity", {
  component: IdentityViewAdapter,
  displayName: "IdentityView",
} as BlockRendererEntry);
