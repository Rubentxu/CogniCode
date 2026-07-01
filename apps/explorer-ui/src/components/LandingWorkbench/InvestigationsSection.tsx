/**
 * InvestigationsSection — Investigation board for the landing workbench.
 *
 * Shows the user's active investigations from the backend and provides
 * quick-start templates as fallbacks when no investigations exist yet.
 *
 * v1 (ADR-005 INV-1): List + create investigations.
 * Future v2: Pin evidence, add artifacts, write narrative.
 */
import { useState } from "react";
import { useAppDispatch } from "../../state/context";
import {
  useInvestigations,
  createInvestigation,
  deleteInvestigation,
} from "../../hooks/useInvestigations";
import type { InvestigationDto } from "../../api/types";

// ADR-005: investigation templates as fallback when no investigations exist
interface InvestigationTemplate {
  id: string;
  label: string;
  description: string;
  goal: string;
}

const INVESTIGATION_TEMPLATES: ReadonlyArray<InvestigationTemplate> = [
  {
    id: "trace-request",
    label: "Trace a request",
    description: "Follow an HTTP route through handlers, use cases, and persistence.",
    goal: "Follow an HTTP route through handlers, use cases, and persistence.",
  },
  {
    id: "impact-radius",
    label: "Find impact radius",
    description: "See who calls a symbol, what depends on it, and what tests cover it.",
    goal: "Find the impact radius of a symbol: callers, callees, and test coverage.",
  },
  {
    id: "understand-ownership",
    label: "Understand ownership",
    description: "Map modules, components, and decisions to their owning concerns.",
    goal: "Map ownership of modules, components, and decisions.",
  },
  {
    id: "review-change-impact",
    label: "Review change impact",
    description: "Trace upstream callers and downstream callees before refactoring.",
    goal: "Trace impact before refactoring: upstream callers and downstream callees.",
  },
  {
    id: "explain-decision",
    label: "Explain a decision",
    description: "Link an ADR or doc to the code that implements it.",
    goal: "Link an ADR or decision to the code that implements it.",
  },
];

function statusColor(status: InvestigationDto["status"]): string {
  switch (status) {
    case "active":
      return "var(--color-primary)";
    case "completed":
      return "var(--color-success, #22c55e)";
    case "archived":
      return "var(--color-text-muted)";
    case "draft":
    default:
      return "var(--color-warning, #f59e0b)";
  }
}

function InvestigationCard({
  investigation,
  onDelete,
}: {
  investigation: InvestigationDto;
  onDelete: (id: string) => void;
}) {
  const timestamp = new Date(investigation.updated_at).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  });

  return (
    <div
      data-testid={`investigation-card-${investigation.id}`}
      style={{
        padding: "12px",
        borderRadius: 8,
        border: "1px solid var(--color-border)",
        backgroundColor: "var(--color-surface-raised)",
        display: "flex",
        flexDirection: "column",
        gap: 6,
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: 8,
        }}
      >
        <span
          style={{
            fontWeight: 600,
            fontSize: 13,
            color: "var(--color-text-primary)",
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
            flex: 1,
          }}
        >
          {investigation.title}
        </span>
        <span
          style={{
            fontSize: 10,
            fontWeight: 500,
            padding: "2px 6px",
            borderRadius: 4,
            backgroundColor: statusColor(investigation.status),
            color: "#fff",
            textTransform: "capitalize",
            flexShrink: 0,
          }}
        >
          {investigation.status}
        </span>
      </div>
      <p
        style={{
          fontSize: 11,
          color: "var(--color-text-muted)",
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
          margin: 0,
        }}
      >
        {investigation.goal}
      </p>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginTop: 4,
        }}
      >
        <span style={{ fontSize: 10, color: "var(--color-text-muted)" }}>
          {timestamp}
          {investigation.evidence.length > 0 && ` · ${investigation.evidence.length} evidence`}
          {investigation.artifacts.length > 0 && ` · ${investigation.artifacts.length} artifacts`}
        </span>
        <button
          type="button"
          data-testid={`delete-investigation-${investigation.id}`}
          onClick={() => onDelete(investigation.id)}
          style={{
            fontSize: 10,
            color: "var(--color-text-muted)",
            background: "none",
            border: "none",
            cursor: "pointer",
            padding: "2px 4px",
          }}
          title="Delete investigation"
        >
          ✕
        </button>
      </div>
    </div>
  );
}

function NewInvestigationForm({
  workspaceId,
  onCancel,
}: {
  workspaceId: string;
  onCancel: () => void;
}) {
  const [title, setTitle] = useState("");
  const [goal, setGoal] = useState("");
  const [isCreating, setIsCreating] = useState(false);

  const handleCreate = async () => {
    if (!title.trim() || !goal.trim()) return;
    setIsCreating(true);
    try {
      await createInvestigation({
        workspace_id: workspaceId,
        title: title.trim(),
        goal: goal.trim(),
      });
      setTitle("");
      setGoal("");
    } finally {
      setIsCreating(false);
    }
  };

  return (
    <div
      data-testid="new-investigation-form"
      style={{
        padding: "12px",
        borderRadius: 8,
        border: "1px solid var(--color-primary)",
        backgroundColor: "var(--color-surface-overlay)",
        display: "flex",
        flexDirection: "column",
        gap: 8,
      }}
    >
      <input
        type="text"
        placeholder="Investigation title"
        value={title}
        onChange={(e) => setTitle(e.target.value)}
        style={{
          padding: "6px 8px",
          borderRadius: 4,
          border: "1px solid var(--color-border)",
          backgroundColor: "var(--color-surface)",
          color: "var(--color-text-primary)",
          fontSize: 12,
          width: "100%",
          boxSizing: "border-box",
        }}
      />
      <textarea
        placeholder="What question are you trying to answer?"
        value={goal}
        onChange={(e) => setGoal(e.target.value)}
        rows={2}
        style={{
          padding: "6px 8px",
          borderRadius: 4,
          border: "1px solid var(--color-border)",
          backgroundColor: "var(--color-surface)",
          color: "var(--color-text-primary)",
          fontSize: 12,
          width: "100%",
          boxSizing: "border-box",
          resize: "vertical",
        }}
      />
      <div style={{ display: "flex", gap: 6 }}>
        <button
          type="button"
          onClick={handleCreate}
          disabled={isCreating || !title.trim() || !goal.trim()}
          data-testid="create-investigation-submit"
          style={{
            padding: "6px 12px",
            borderRadius: 4,
            border: "none",
            backgroundColor:
              isCreating || !title.trim() || !goal.trim()
                ? "var(--color-border)"
                : "var(--color-primary)",
            color:
              isCreating || !title.trim() || !goal.trim()
                ? "var(--color-text-muted)"
                : "var(--color-primary-foreground, #fff)",
            fontSize: 12,
            fontWeight: 500,
            cursor:
              isCreating || !title.trim() || !goal.trim() ? "not-allowed" : "pointer",
          }}
        >
          {isCreating ? "Creating…" : "Create"}
        </button>
        <button
          type="button"
          onClick={onCancel}
          data-testid="create-investigation-cancel"
          style={{
            padding: "6px 12px",
            borderRadius: 4,
            border: "1px solid var(--color-border)",
            backgroundColor: "transparent",
            color: "var(--color-text-secondary)",
            fontSize: 12,
            cursor: "pointer",
          }}
        >
          Cancel
        </button>
      </div>
    </div>
  );
}

interface InvestigationsSectionProps {
  workspaceId: string;
}

export function InvestigationsSection({ workspaceId }: InvestigationsSectionProps) {
  const dispatch = useAppDispatch();
  const { data: investigations, isLoading } = useInvestigations(workspaceId);
  const [showNewForm, setShowNewForm] = useState(false);

  const activeInvestigations =
    investigations?.filter((inv) => inv.status === "active" || inv.status === "draft") ?? [];
  const completedInvestigations =
    investigations?.filter((inv) => inv.status === "completed" || inv.status === "archived") ?? [];

  const handleDelete = async (id: string) => {
    if (!window.confirm("Delete this investigation? This cannot be undone.")) return;
    await deleteInvestigation(id);
  };

  const handleTemplateClick = (goal: string) => {
    // v1: just open Spotter. Future: create investigation with pre-filled goal.
    dispatch({
      type: "SET_SPOTTER",
      payload: { open: true },
    });
    window.dispatchEvent(
      new CustomEvent("cognicode:investigation-hint", { detail: goal }),
    );
  };

  return (
    <div
      data-testid="investigations-section"
      className="flex flex-col gap-4 p-6"
      aria-label="Investigation board"
      style={{ overflow: "auto" }}
    >
      <header>
        <h2
          className="text-sm font-semibold"
          style={{ color: "var(--color-text-primary)" }}
        >
          Investigations
        </h2>
        <p
          className="mt-1 text-xs"
          style={{ color: "var(--color-text-muted)" }}
        >
          Focused exploration sessions with evidence and narrative.
        </p>
      </header>

      {/* New investigation form */}
      {showNewForm ? (
        <NewInvestigationForm
          workspaceId={workspaceId}
          onCancel={() => setShowNewForm(false)}
        />
      ) : (
        <button
          type="button"
          data-testid="new-investigation-button"
          onClick={() => setShowNewForm(true)}
          style={{
            padding: "8px 12px",
            borderRadius: 6,
            border: "1px dashed var(--color-border)",
            backgroundColor: "transparent",
            color: "var(--color-text-secondary)",
            fontSize: 12,
            cursor: "pointer",
            textAlign: "left",
            transition: "border-color 0.15s, color 0.15s",
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.borderColor = "var(--color-primary)";
            e.currentTarget.style.color = "var(--color-primary)";
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.borderColor = "var(--color-border)";
            e.currentTarget.style.color = "var(--color-text-secondary)";
          }}
        >
          + New Investigation
        </button>
      )}

      {/* Active investigations */}
      {!isLoading && activeInvestigations.length > 0 && (
        <section>
          <h3
            style={{
              fontSize: 11,
              fontWeight: 600,
              color: "var(--color-text-muted)",
              textTransform: "uppercase",
              letterSpacing: "0.05em",
              marginBottom: 8,
            }}
          >
            Active
          </h3>
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "repeat(auto-fill, minmax(240px, 1fr))",
              gap: 8,
            }}
          >
            {activeInvestigations.map((inv) => (
              <InvestigationCard
                key={inv.id}
                investigation={inv}
                onDelete={handleDelete}
              />
            ))}
          </div>
        </section>
      )}

      {/* Completed investigations */}
      {!isLoading && completedInvestigations.length > 0 && (
        <section>
          <h3
            style={{
              fontSize: 11,
              fontWeight: 600,
              color: "var(--color-text-muted)",
              textTransform: "uppercase",
              letterSpacing: "0.05em",
              marginBottom: 8,
            }}
          >
            Completed
          </h3>
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "repeat(auto-fill, minmax(240px, 1fr))",
              gap: 8,
            }}
          >
            {completedInvestigations.map((inv) => (
              <InvestigationCard
                key={inv.id}
                investigation={inv}
                onDelete={handleDelete}
              />
            ))}
          </div>
        </section>
      )}

      {/* Templates fallback — shown when no investigations exist */}
      {!isLoading && (activeInvestigations.length === 0 && completedInvestigations.length === 0) && (
        <section>
          <h3
            style={{
              fontSize: 11,
              fontWeight: 600,
              color: "var(--color-text-muted)",
              textTransform: "uppercase",
              letterSpacing: "0.05em",
              marginBottom: 8,
            }}
          >
            Quick start
          </h3>
          <ul className="flex flex-col gap-2">
            {INVESTIGATION_TEMPLATES.map((tpl) => (
              <li key={tpl.id}>
                <button
                  type="button"
                  data-testid={`investigation-template-${tpl.id}`}
                  onClick={() => handleTemplateClick(tpl.goal)}
                  className="w-full rounded-md border px-4 py-3 text-left transition-colors"
                  style={{
                    borderColor: "var(--color-border)",
                    backgroundColor: "var(--color-surface-raised)",
                  }}
                >
                  <div
                    className="text-sm font-medium"
                    style={{ color: "var(--color-text-primary)" }}
                  >
                    {tpl.label}
                  </div>
                  <p
                    className="mt-1 text-xs"
                    style={{ color: "var(--color-text-muted)" }}
                  >
                    {tpl.description}
                  </p>
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}

      {/* Loading state */}
      {isLoading && (
        <p
          style={{
            fontSize: 12,
            color: "var(--color-text-muted)",
            textAlign: "center",
            padding: "16px 0",
          }}
        >
          Loading investigations…
        </p>
      )}
    </div>
  );
}
