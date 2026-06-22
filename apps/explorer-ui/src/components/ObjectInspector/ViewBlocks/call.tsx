/**
 * Call-related block renderers: CallListView, CallListItemRow,
 * CallMetricsView, SignatureView.
 */
import { useMemo } from "react";
import type {
  CallListBlockBody,
  RelationItem,
  SourceLine,
  SourceSliceBlockBody,
  ViewBlock,
} from "../../../api/types";
import { detectLanguage } from "../../../utils/languageDetect";
import {
  tokenizePrism,
  splitTokensByNewline,
} from "../../../utils/highlight-core";
import { renderTokens } from "../../../utils/highlight";
import { BlockShell, Stat } from "./shared";
import type { CallListProps } from "./types";
import {
  type BlockRendererEntry,
  type BlockRendererProps,
  registerBlockRenderer,
} from "../blockRendererRegistry";

// ============================================================================
// CallMetricsView
// ============================================================================

export function CallMetricsView({
  block,
}: {
  block: ViewBlock & { body: { fan_in: number; fan_out: number } };
}) {
  const b = block.body;
  return (
    <BlockShell id={block.id} title={block.title}>
      <dl className="grid grid-cols-2 gap-2 text-sm">
        <Stat label="Fan in" value={b.fan_in} />
        <Stat label="Fan out" value={b.fan_out} />
      </dl>
    </BlockShell>
  );
}

// ============================================================================
// SignatureView
// ============================================================================

// file prop used in T3.2 for language detection via detectLanguage
export function SignatureView({
  block,
  file: _file,
}: {
  block: ViewBlock & { body: { signature: string } };
  file?: string;
}) {
  void _file; // T3.2: will use file for detectLanguage call
  return (
    <BlockShell id={block.id} title={block.title}>
      <pre
        tabIndex={0}
        className="overflow-x-auto rounded-sm p-2 font-mono text-xs"
        style={{
          backgroundColor: "var(--color-surface-overlay)",
          color: "var(--color-text-primary)",
        }}
      >
        <code>{block.body.signature}</code>
      </pre>
    </BlockShell>
  );
}

// ============================================================================
// CallListView + CallListItemRow
// ============================================================================

export function CallListView({ block, onSelectObject }: CallListProps) {
  const items = block.body.items;
  if (items.length === 0) {
    return (
      <BlockShell id={block.id} title={block.title}>
        <p style={{ color: "var(--color-text-muted)" }}>No items.</p>
      </BlockShell>
    );
  }
  return (
    <BlockShell id={block.id} title={block.title}>
      <ul
        data-testid={`view-block-${block.id}-items`}
        className="flex flex-col gap-0.5"
      >
        {items.map((item: RelationItem) => (
          <CallListItemRow
            key={item.object_id}
            item={item}
            onSelectObject={onSelectObject}
          />
        ))}
      </ul>
    </BlockShell>
  );
}

function CallListItemRow({
  item,
  onSelectObject,
}: {
  item: RelationItem;
  onSelectObject?: (id: string) => void;
}) {
  const interactive = Boolean(onSelectObject);
  // When interactive we render a <button> INSIDE the <li>. Putting
  // `role="button"` directly on an <li> breaks the list semantics
  // (axe: "List element has direct children that are not allowed").
  // The <li> stays a list item; the <button> carries the focusable
  // affordance.
  return (
    <li
      data-testid={`view-block-item-${item.object_id}`}
      className="list-none"
    >
      {interactive ? (
        <button
          type="button"
          onClick={() => onSelectObject?.(item.object_id)}
          data-testid={`view-block-item-button-${item.object_id}`}
          className="flex w-full cursor-pointer items-center gap-2 rounded-sm px-2 py-1 text-left text-sm"
          style={{
            backgroundColor: "transparent",
            color: "var(--color-text-primary)",
          }}
        >
          <span
            aria-hidden="true"
            className="inline-flex h-4 w-4 flex-none items-center justify-center font-mono text-xs"
            style={{ color: "var(--color-text-muted)" }}
          >
            ƒ
          </span>
          <span className="min-w-0 flex-1 truncate" title={item.name}>
            {item.name}
          </span>
          <span
            className="font-mono text-xs"
            style={{ color: "var(--color-text-muted)" }}
          >
            {item.file}:{item.line}
          </span>
        </button>
      ) : (
        <div
          className="flex items-center gap-2 rounded-sm px-2 py-1 text-sm"
          style={{ color: "var(--color-text-primary)" }}
        >
          <span
            aria-hidden="true"
            className="inline-flex h-4 w-4 flex-none items-center justify-center font-mono text-xs"
            style={{ color: "var(--color-text-muted)" }}
          >
            ƒ
          </span>
          <span className="min-w-0 flex-1 truncate" title={item.name}>
            {item.name}
          </span>
          <span
            className="font-mono text-xs"
            style={{ color: "var(--color-text-muted)" }}
          >
            {item.file}:{item.line}
          </span>
        </div>
      )}
    </li>
  );
}

// ============================================================================
// SourceView (source_slice)
// ============================================================================

export function SourceView({
  block,
}: {
  block: ViewBlock & { body: SourceSliceBlockBody };
}) {
  const b = block.body;

  // Join all lines once, then tokenize and split per line.
  // This correctly handles multiline tokens (e.g. /* block comments */)
  // while preserving the original text content for getByText() survival.
  const joinedText = useMemo(
    () => b.lines.map((l: SourceLine) => l.text).join("\n"),
    [b.lines],
  );
  const detectedLang = detectLanguage(b.file) ?? undefined;
  const { tokens } = useMemo(
    () => tokenizePrism(joinedText, detectedLang),
    [joinedText, detectedLang],
  );
  const perLineTokens = useMemo(
    () => splitTokensByNewline(tokens, b.lines.length),
    [tokens, b.lines.length],
  );

  return (
    <BlockShell id={block.id} title={block.title}>
      <p
        className="font-mono text-xs"
        style={{ color: "var(--color-text-muted)" }}
      >
        {b.file} · starting at line {b.line}
      </p>
      <ol
        className="mt-2 flex flex-col font-mono text-xs"
        style={{
          backgroundColor: "var(--color-surface-overlay)",
          color: "var(--color-text-primary)",
          borderRadius: "var(--radius-sm)",
          overflow: "hidden",
        }}
      >
        {b.lines.map((ln: SourceLine, idx: number) => {
          const lineTokens = perLineTokens[idx];
          return (
            <li
              key={ln.line}
              data-testid={`source-line-${ln.line}`}
              className="flex"
            >
              <span
                aria-hidden="true"
                className="select-none px-2 py-0.5 text-right"
                style={{
                  width: "3.5rem",
                  color: "var(--color-text-muted)",
                  borderRight: "1px solid var(--color-border)",
                }}
              >
                {ln.line}
              </span>
              <span className="flex-1 whitespace-pre px-2 py-0.5">
                {lineTokens && lineTokens.length > 0
                  ? renderTokens(lineTokens, `src-${ln.line}-`)
                  : ln.text || " "}
              </span>
            </li>
          );
        })}
      </ol>
    </BlockShell>
  );
}

// ============================================================================
// Registry adapters — interactive blocks need onSelectObject from extra
// ============================================================================

// Extra type for interactive blocks that need onSelectObject
type InteractiveExtra = { onSelectObject?: (id: string) => void };

function CallListViewAdapter({ block, extra }: BlockRendererProps<InteractiveExtra>) {
  return (
    <CallListView
      block={block as ViewBlock & { body: CallListBlockBody }}
      onSelectObject={extra?.onSelectObject}
    />
  );
}

// Register callers and callees (both use CallListView)
registerBlockRenderer("callers", {
  component: CallListViewAdapter,
  displayName: "CallListView (callers)",
} as BlockRendererEntry);

registerBlockRenderer("callees", {
  component: CallListViewAdapter,
  displayName: "CallListView (callees)",
} as BlockRendererEntry);

// Non-interactive adapters
function CallMetricsViewAdapter({ block }: BlockRendererProps) {
  return (
    <CallMetricsView
      block={block as ViewBlock & { body: { fan_in: number; fan_out: number } }}
    />
  );
}

type SignatureExtra = { file?: string; onSelectObject?: (id: string) => void };

function SignatureViewAdapter({ block, extra }: BlockRendererProps<SignatureExtra>) {
  return (
    <SignatureView
      block={block as ViewBlock & { body: { signature: string } }}
      file={extra?.file}
    />
  );
}

function SourceViewAdapter({ block }: BlockRendererProps) {
  return (
    <SourceView
      block={block as ViewBlock & { body: SourceSliceBlockBody }}
    />
  );
}

registerBlockRenderer("call_metrics", {
  component: CallMetricsViewAdapter,
  displayName: "CallMetricsView",
} as BlockRendererEntry);

registerBlockRenderer("signature", {
  component: SignatureViewAdapter,
  displayName: "SignatureView",
} as BlockRendererEntry);

registerBlockRenderer("source_slice", {
  component: SourceViewAdapter,
  displayName: "SourceView",
} as BlockRendererEntry);
