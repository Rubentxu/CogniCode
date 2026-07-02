/**
 * `ScaffoldPicker` — Scaffold-first step for the ViewSpecWizard.
 *
 * Shows scaffolds grouped by `object_type` (filtered to the current
 * `objectType`). The user picks one, or opts for a custom MoldQL query.
 *
 * When a scaffold is selected, the `onSelect` callback receives:
 *   - `viewKind` — recommended `ViewKind`
 *   - `rendererKind` — recommended `RendererKind`
 *   - `query` — the `query_template` with `{{object_id}}` substituted
 *
 * This step is skipped when the user chooses "Custom query".
 *
 * @see `useScaffoldRegistry` for scaffold filtering by object type.
 * @see `ViewSpecWizard` for the wizard integration.
 */
import { useCallback, useMemo, useState } from "react";

import { useScaffoldRegistry } from "../../hooks/useScaffoldRegistry";
import type { Scaffold } from "../../api/scaffoldSchema";
import type { InspectableObjectType } from "../../api/types";
import type { RendererKind, ViewKind } from "../../api/schemas";

// ============================================================================
// Props
// ============================================================================

export interface ScaffoldPickerProps {
  /** The inspectable object type being inspected. */
  objectType: InspectableObjectType;
  /** The id of the focused object (substituted into `{{object_id}}`). */
  objectId: string;
  /** Called when a scaffold is selected. */
  onSelect: (result: ScaffoldSelection) => void;
  /** Called when the user opts for a custom MoldQL query. */
  onCustomQuery: () => void;
}

export interface ScaffoldSelection {
  scaffoldId: string;
  viewKind: ViewKind;
  rendererKind: RendererKind;
  query: string;
}

// ============================================================================
// Component
// ============================================================================

export function ScaffoldPicker({
  objectType,
  objectId,
  onSelect,
  onCustomQuery,
}: ScaffoldPickerProps): React.ReactElement {
  const [search, setSearch] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const allScaffolds = useScaffoldRegistry(objectType);

  /** Group scaffolds by their object_type for display. */
  const groupedScaffolds = useMemo(() => {
    const scaffolds = search.trim()
      ? allScaffolds.filter(
          (s) =>
            s.label.toLowerCase().includes(search.toLowerCase()) ||
            s.intent.toLowerCase().includes(search.toLowerCase()),
        )
      : allScaffolds;

    // Group by object_type (though in wizard context we filter to one type,
    // keeping group structure for consistency).
    const byType = new Map<string, Scaffold[]>();
    for (const s of scaffolds) {
      const list = byType.get(s.object_type) ?? [];
      list.push(s);
      byType.set(s.object_type, list);
    }
    return byType;
  }, [allScaffolds, search]);

  const selectedScaffold = useMemo(
    () => allScaffolds.find((s) => s.id === selectedId) ?? null,
    [allScaffolds, selectedId],
  );

  const handleSelect = useCallback(() => {
    if (!selectedScaffold) return;
    const query = selectedScaffold.query_template.replace(/\{\{object_id\}\}/g, objectId);
    onSelect({
      scaffoldId: selectedScaffold.id,
      viewKind: selectedScaffold.view_kind,
      rendererKind: selectedScaffold.renderer_kind,
      query,
    });
  }, [selectedScaffold, objectId, onSelect]);

  return (
    <div className="flex flex-col gap-4">
      <div>
        <h3 className="mb-1 text-sm font-medium" style={{ color: "var(--color-text-primary)" }}>
          Choose a Scaffold
        </h3>
        <p className="text-xs" style={{ color: "var(--color-text-muted)" }}>
          Pick a scaffold to pre-fill the query, or choose a custom MoldQL query.
        </p>
      </div>

      {/* Search */}
      <div className="relative">
        <input
          type="search"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Search scaffolds…"
          className="w-full rounded-md px-3 py-2 text-sm"
          style={{
            backgroundColor: "var(--color-surface-overlay)",
            color: "var(--color-text-primary)",
            border: "1px solid var(--color-border)",
          }}
        />
      </div>

      {/* Scaffold list */}
      <div className="flex flex-col gap-3" style={{ maxHeight: "320px", overflowY: "auto" }}>
        {allScaffolds.length === 0 && (
          <p className="text-xs" style={{ color: "var(--color-text-muted)" }}>
            No scaffolds available for this object type.
          </p>
        )}
        {Array.from(groupedScaffolds.entries()).map(([objectType, scaffolds]) => (
          <div key={objectType}>
            <h4
              className="mb-2 text-[10px] font-semibold uppercase tracking-widest"
              style={{ color: "var(--color-text-muted)" }}
            >
              {objectType}
            </h4>
            <div className="flex flex-col gap-2">
              {scaffolds.map((scaffold) => {
                const isSelected = selectedId === scaffold.id;
                return (
                  <button
                    key={scaffold.id}
                    type="button"
                    onClick={() => setSelectedId(scaffold.id)}
                    className="rounded-md p-3 text-left text-sm transition-colors"
                    style={{
                      backgroundColor: isSelected
                        ? "var(--color-primary)"
                        : "var(--color-surface-overlay)",
                      color: isSelected
                        ? "var(--color-primary-foreground)"
                        : "var(--color-text-primary)",
                      border: isSelected
                        ? "2px solid var(--color-primary)"
                        : "1px solid var(--color-border)",
                    }}
                  >
                    <div className="flex flex-col gap-1">
                      <span className="font-medium">{scaffold.label}</span>
                      <span
                        className="text-xs"
                        style={{
                          color: isSelected
                            ? "var(--color-primary-foreground)"
                            : "var(--color-text-muted)",
                          opacity: isSelected ? 0.8 : 1,
                        }}
                      >
                        {scaffold.description}
                      </span>
                    </div>
                  </button>
                );
              })}
            </div>
          </div>
        ))}
      </div>

      {/* Preview of selected scaffold's query */}
      {selectedScaffold && (
        <div
          className="rounded-md p-3 text-xs font-mono"
          style={{
            backgroundColor: "var(--color-surface-overlay)",
            color: "var(--color-text-secondary)",
            border: "1px solid var(--color-border)",
          }}
        >
          <strong>Query preview:</strong>
          <pre className="mt-1 whitespace-pre-wrap break-all">
            {selectedScaffold.query_template.replace(/\{\{object_id\}\}/g, objectId)}
          </pre>
        </div>
      )}

      {/* Actions */}
      <div className="flex gap-3">
        <button
          type="button"
          onClick={handleSelect}
          disabled={!selectedScaffold}
          className="rounded-md px-4 py-2 text-sm font-medium transition-colors disabled:opacity-40"
          style={{
            backgroundColor: selectedScaffold ? "var(--color-primary)" : "var(--color-surface-overlay)",
            color: selectedScaffold ? "var(--color-primary-foreground)" : "var(--color-text-muted)",
          }}
        >
          Use Scaffold
        </button>
        <button
          type="button"
          onClick={onCustomQuery}
          className="rounded-md px-4 py-2 text-sm font-medium transition-colors"
          style={{
            backgroundColor: "var(--color-surface-overlay)",
            color: "var(--color-text-primary)",
          }}
        >
          Custom Query
        </button>
      </div>
    </div>
  );
}
