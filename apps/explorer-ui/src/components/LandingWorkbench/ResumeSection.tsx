/**
 * ResumeSection — wraps the existing RecentExplorationsStrip.
 *
 * Reuses the proven RecentExplorationsStrip component without
 * modification. This is the "Resume previous work" surface of the
 * LandingWorkbench.
 */
import { useAppDispatch } from "../../state/context";
import { RecentExplorationsStrip } from "../GraphLanding/RecentExplorationsStrip";
import type { ExplorationSessionDto } from "../../api/types";

export interface ResumeSectionProps {
  workspaceId: string;
}

export function ResumeSection({ workspaceId }: ResumeSectionProps) {
  const dispatch = useAppDispatch();

  const handleExplorationClick = (exploration: ExplorationSessionDto) => {
    // Reuse the same dispatch pattern as RecentExplorationsStrip
    const firstPane = exploration.panes[0];
    const firstEvent = exploration.events[0];
    const objectId = firstPane?.object_id ?? firstEvent?.object_id;
    if (objectId) {
      dispatch({
        type: "SELECT_OBJECT",
        payload: { objectId, viewId: firstPane?.view_id ?? undefined },
      });
    }
  };

  return (
    <div
      data-testid="resume-section"
      className="flex flex-col gap-4 p-6"
      aria-label="Resume previous explorations"
    >
      <header>
        <h2
          className="text-sm font-semibold"
          style={{ color: "var(--color-text-primary)" }}
        >
          Resume
        </h2>
        <p
          className="mt-1 text-xs"
          style={{ color: "var(--color-text-muted)" }}
        >
          Pick up where you left off.
        </p>
      </header>
      <RecentExplorationsStrip
        workspaceId={workspaceId}
        onExplorationClick={handleExplorationClick}
      />
    </div>
  );
}
