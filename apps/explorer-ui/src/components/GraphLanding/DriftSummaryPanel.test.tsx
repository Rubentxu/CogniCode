/**
 * `DriftSummaryPanel` tests.
 */
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import type { DriftReport } from "../../api/types";
import { DriftSummaryPanel } from "./DriftSummaryPanel";

const mockDriftReport = (findings: DriftReport["findings"]): DriftReport => ({
  findings,
  summary: "test summary",
  missing_containers: findings.filter((f) => f.kind === "missing").length,
  extra_containers: findings.filter((f) => f.kind === "extra").length,
  wrong_sub_kinds: findings.filter((f) => f.kind === "wrong_sub_kind").length,
  boundary_violations: 0,
});

describe("DriftSummaryPanel", () => {
  it("shows unmatched findings", () => {
    const report = mockDriftReport([
      { kind: "missing", expected: "api-gateway", actual: "—", severity: "high", detail: "" },
      { kind: "extra", expected: "—", actual: "orphan-service", severity: "medium", detail: "" },
    ]);

    render(<DriftSummaryPanel driftReport={report} c4NodeLabels={["web-frontend", "backend-db"]} />);

    expect(screen.getByTestId("c4-drift-summary")).toBeInTheDocument();
    expect(screen.getByText("Drift Findings (2)")).toBeInTheDocument();
    expect(screen.getByText("api-gateway")).toBeInTheDocument();
    expect(screen.getByText("orphan-service")).toBeInTheDocument();
  });

  it("omits matched findings", () => {
    const report = mockDriftReport([
      { kind: "missing", expected: "api-gateway", actual: "—", severity: "high", detail: "" },
      { kind: "extra", expected: "—", actual: "orphan-service", severity: "medium", detail: "" },
    ]);

    // "api-gateway" is in the visible set, so only orphan-service should show
    render(<DriftSummaryPanel driftReport={report} c4NodeLabels={["api-gateway", "backend-db"]} />);

    expect(screen.getByTestId("c4-drift-summary")).toBeInTheDocument();
    expect(screen.getByText("Drift Findings (1)")).toBeInTheDocument();
    expect(screen.queryByText("api-gateway")).not.toBeInTheDocument();
    expect(screen.getByText("orphan-service")).toBeInTheDocument();
  });

  it("shows 0 items when all findings are matched", () => {
    const report = mockDriftReport([
      { kind: "missing", expected: "api-gateway", actual: "—", severity: "high", detail: "" },
      { kind: "extra", expected: "—", actual: "orphan-service", severity: "medium", detail: "" },
    ]);

    // Both findings match nodes in the visible set
    render(<DriftSummaryPanel driftReport={report} c4NodeLabels={["api-gateway", "orphan-service"]} />);

    expect(screen.queryByTestId("c4-drift-summary")).not.toBeInTheDocument();
  });

  it("renders finding type chips correctly", () => {
    const report = mockDriftReport([
      { kind: "missing", expected: "service-a", actual: "—", severity: "high", detail: "" },
      { kind: "extra", expected: "—", actual: "service-b", severity: "medium", detail: "" },
      { kind: "wrong_sub_kind", expected: "service-c", actual: "service-c-wrong", severity: "low", detail: "" },
    ]);

    render(<DriftSummaryPanel driftReport={report} c4NodeLabels={[]} />);

    expect(screen.getByTestId("c4-drift-summary")).toBeInTheDocument();
    expect(screen.getByText("Drift Findings (3)")).toBeInTheDocument();
    // Chips use replace("_", " ") so "wrong_sub_kind" becomes "wrong sub kind"
    expect(screen.getByText("missing")).toBeInTheDocument();
    expect(screen.getByText("extra")).toBeInTheDocument();
    expect(screen.getByText("wrong sub kind")).toBeInTheDocument();
  });

  it("returns null when driftReport has no findings", () => {
    const report = mockDriftReport([]);
    const { container } = render(<DriftSummaryPanel driftReport={report} c4NodeLabels={["some-node"]} />);
    expect(container.firstChild).toBeNull();
  });
});
