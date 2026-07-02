/**
 * `AffordanceCards` — renders typed affordance cards for an inspectable object.
 *
 * Shown in PaneInspector when no view is loaded yet, providing quick
 * navigation to common views for the object type.
 */
import type { Affordance } from "../../api/affordanceSchema";

interface AffordanceCardsProps {
  objectId: string;
  objectLabel: string;
  affordances: Affordance[];
  onSelectAffordance: (aff: Affordance) => void;
}

export function AffordanceCards({
  objectLabel,
  affordances,
  onSelectAffordance,
}: AffordanceCardsProps): React.ReactElement {
  return (
    <div className="space-y-3">
      <p className="text-sm font-medium" style={{ color: "var(--color-text-secondary)" }}>
        Quick views for {objectLabel}
      </p>
      <div className="grid gap-2">
        {affordances.map((aff) => (
          <button
            key={`${aff.object_type}-${aff.view_kind}-${aff.priority}`}
            type="button"
            onClick={() => onSelectAffordance(aff)}
            className="flex items-start gap-3 rounded-lg border p-3 text-left transition-colors hover:bg-[var(--color-surface-overlay)]"
            style={{
              borderColor: "var(--color-border)",
              backgroundColor: "var(--color-surface)",
            }}
          >
            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-2">
                <span className="text-sm font-medium truncate" style={{ color: "var(--color-text-primary)" }}>
                  {aff.label}
                </span>
                {aff.scaffold_id && (
                  <span
                    className="rounded-full px-1.5 py-0.5 text-[10px] font-medium"
                    style={{
                      backgroundColor: "var(--color-surface-overlay)",
                      color: "var(--color-text-muted)",
                    }}
                  >
                    {aff.scaffold_id}
                  </span>
                )}
              </div>
              <p className="mt-0.5 text-xs line-clamp-2" style={{ color: "var(--color-text-muted)" }}>
                {aff.description}
              </p>
            </div>
            <span
              className="rounded-full px-2 py-0.5 text-[10px] font-medium"
              style={{
                backgroundColor: "var(--color-surface-overlay)",
                color: "var(--color-text-muted)",
              }}
            >
              {aff.view_kind}
            </span>
          </button>
        ))}
      </div>
    </div>
  );
}
