/**
 * StartFromSection — entry point types grid for the LandingWorkbench.
 *
 * Shows 5 entry point types (Route, Use case, Symbol, Event, Saved
 * exploration) as clickable cards. Each card opens Spotter with the
 * matching kind chip pre-selected.
 */
import { useAppDispatch } from "../../state/context";
import { ENTRY_POINT_TYPES, type EntryPointType } from "./entryPointTypes";

export function StartFromSection() {
  const dispatch = useAppDispatch();

  const handleClick = (entry: EntryPointType) => {
    dispatch({
      type: "SET_SPOTTER",
      payload: { open: true, kind: entry.spotterKind },
    });
  };

  return (
    <div
      data-testid="start-from-section"
      className="flex flex-col gap-4 p-6"
      aria-label="Start an investigation from"
    >
      <header>
        <h2
          className="text-sm font-semibold"
          style={{ color: "var(--color-text-primary)" }}
        >
          Start from
        </h2>
        <p
          className="mt-1 text-xs"
          style={{ color: "var(--color-text-muted)" }}
        >
          Pick a kind of thing to investigate. Spotter opens with the matching filter.
        </p>
      </header>
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {ENTRY_POINT_TYPES.map((entry) => (
          <button
            key={entry.id}
            type="button"
            data-testid={`entry-point-${entry.id}`}
            onClick={() => handleClick(entry)}
            className="flex items-start gap-3 rounded-lg border p-4 text-left transition-colors"
            style={{
              borderColor: "var(--color-border)",
              backgroundColor: "var(--color-surface-raised)",
            }}
          >
            <span
              aria-hidden="true"
              className="text-2xl"
              style={{ color: "var(--color-accent)" }}
            >
              {entry.icon}
            </span>
            <div>
              <div
                className="text-sm font-medium"
                style={{ color: "var(--color-text-primary)" }}
              >
                {entry.label}
              </div>
              <p
                className="mt-1 text-xs"
                style={{ color: "var(--color-text-muted)" }}
              >
                {entry.description}
              </p>
            </div>
          </button>
        ))}
      </div>
    </div>
  );
}
