import { useAppDispatch, useAppState } from "../state/context";
import { useObject } from "../hooks/useObject";
import { ViewSpecWizardTrigger } from "./ViewSpecWizardTrigger";

function objectTypeLabel(kind: string | undefined): string {
  switch (kind) {
    case "decision_artifact":
      return "Decision";
    case "quality_issue":
      return "Quality issue";
    case "saved_exploration":
      return "Saved exploration";
    case "use_case":
      return "Use case";
    default:
      return kind?.replaceAll("_", " ") ?? "Object";
  }
}

export function ContextRail() {
  const dispatch = useAppDispatch();
  const { activeObjectId, navigation, activeInvestigationId } = useAppState();
  const activePane = navigation.panes.find((pane) => pane.id === navigation.activePaneId) ?? null;
  const { data: object } = useObject(activeObjectId);

  return (
    <aside
      data-testid="context-rail"
      className="hidden h-full w-80 flex-shrink-0 flex-col border-l lg:flex"
      style={{
        borderColor: "var(--color-border)",
        backgroundColor: "var(--color-surface-raised)",
      }}
      aria-label="Context rail"
    >
      <div className="border-b px-4 py-4" style={{ borderColor: "var(--color-border)" }}>
        <p
          className="text-[11px] font-semibold uppercase tracking-[0.08em]"
          style={{ color: "var(--color-text-muted)" }}
        >
          Active context
        </p>
        {activeObjectId ? (
          <>
            <h2 className="mt-2 text-sm font-semibold" style={{ color: "var(--color-text-primary)" }}>
              {object?.label ?? activeObjectId}
            </h2>
            <p className="mt-1 text-xs leading-5" style={{ color: "var(--color-text-secondary)" }}>
              {objectTypeLabel(object?.object_type)}
              {activePane?.viaViewKind ? ` · via ${activePane.viaViewKind}` : ""}
            </p>
          </>
        ) : (
          <p className="mt-2 text-xs leading-5" style={{ color: "var(--color-text-secondary)" }}>
            Select an object to reveal related knowledge, actions, and
            continuation paths.
          </p>
        )}
      </div>

      <div className="flex flex-col gap-6 overflow-auto px-4 py-4 text-sm">
        <section>
          <h3 className="text-xs font-semibold uppercase tracking-[0.08em]" style={{ color: "var(--color-text-muted)" }}>
            Continue
          </h3>
          <div className="mt-3 flex flex-col gap-2">
            <button
              type="button"
              data-testid="context-rail-open-spotter"
              onClick={() => dispatch({ type: "SET_SPOTTER", payload: { open: true } })}
              className="rounded-lg border px-3 py-2 text-left text-xs font-medium"
              style={{
                borderColor: "var(--color-border)",
                backgroundColor: "var(--color-surface)",
                color: "var(--color-text-primary)",
              }}
            >
              Open Spotter
            </button>
            <ViewSpecWizardTrigger />
          </div>
        </section>

        <section>
          {/* E27.3-pending: This section is a structural stub. Real knowledge
              content (ADRs/evidence/decisions linked to the active object)
              is deferred to E27.3. */}
          <h3 className="text-xs font-semibold uppercase tracking-[0.08em]" style={{ color: "var(--color-text-muted)" }}>
            Knowledge
          </h3>
          <div className="mt-3 space-y-2 rounded-xl border px-3 py-3" style={{
            borderColor: "var(--color-border)",
            backgroundColor: "var(--color-surface)",
          }}>
            <p className="text-xs leading-5" style={{ color: "var(--color-text-secondary)" }}>
              This rail will surface linked ADRs, docs, evidence, artifacts, and
              next-step actions for the active object.
            </p>
            {activeInvestigationId ? (
              <p className="text-xs" style={{ color: "var(--color-text-primary)" }}>
                Investigation active: <span className="font-medium">{activeInvestigationId}</span>
              </p>
            ) : (
              <p className="text-xs" style={{ color: "var(--color-text-muted)" }}>
                No active investigation yet.
              </p>
            )}
          </div>
        </section>

        <section>
          <h3 className="text-xs font-semibold uppercase tracking-[0.08em]" style={{ color: "var(--color-text-muted)" }}>
            Pane actions
          </h3>
          <ul className="mt-3 space-y-2 text-xs leading-5" style={{ color: "var(--color-text-secondary)" }}>
            <li>Pin evidence and export actions stay attached to the active pane header.</li>
            <li>Pane notes explain why a pane exists and preserve exploration intent.</li>
            <li>View tabs switch representation of the same object, not the screen.</li>
          </ul>
        </section>
      </div>
    </aside>
  );
}
