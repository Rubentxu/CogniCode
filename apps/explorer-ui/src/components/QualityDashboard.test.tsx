/**
 * `QualityDashboard` tests — Phase 10 acceptance criteria.
 *
 * 1. Renders the rating + debt + total + per-severity counts.
 * 2. Renders five severity filter chips with the right colors.
 * 3. Clicking a severity chip filters the issues list.
 * 4. Clicking the active chip again clears the filter.
 * 5. The empty state appears when no issues match the filter.
 */
import { describe, it, expect } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { QualityDashboard } from "./QualityDashboard";
import type {
  QualityIssueItem,
  QualitySummaryBlockBody,
} from "../api/types";

const SUMMARY: QualitySummaryBlockBody = {
  scope: "crates/cognicode-explorer/src",
  rating: "B",
  total_issues: 12,
  debt_minutes: 84,
  by_severity: {
    blocker: 1,
    critical: 2,
    major: 3,
    minor: 4,
    info: 2,
  },
  last_run: "2026-06-07T09:00:00Z",
};

const ISSUES: QualityIssueItem[] = [
  {
    id: 1,
    rule_id: "rust:S100",
    severity: "blocker",
    category: "naming",
    file: "src/lib.rs",
    line: 16,
    message: "Blocker issue",
    status: "open",
    object_id: "issue:1",
  },
  {
    id: 2,
    rule_id: "rust:S200",
    severity: "critical",
    category: "safety",
    file: "src/lib.rs",
    line: 17,
    message: "Critical issue",
    status: "open",
    object_id: "issue:2",
  },
  {
    id: 3,
    rule_id: "rust:S300",
    severity: "info",
    category: "style",
    file: "src/lib.rs",
    line: 18,
    message: "Info issue",
    status: "open",
    object_id: "issue:3",
  },
];

describe("QualityDashboard", () => {
  it("renders the rating, total, debt, and last run", () => {
    render(<QualityDashboard summary={SUMMARY} issues={ISSUES} />);
    const dash = screen.getByTestId("quality-dashboard");
    expect(within(dash).getByTestId("quality-dashboard-rating")).toHaveTextContent("B");
    expect(within(dash).getByText("12")).toBeInTheDocument();
    expect(within(dash).getByText(/84 min/)).toBeInTheDocument();
    expect(within(dash).getByText("09:00:00")).toBeInTheDocument();
  });

  it("renders all five severity chips with the right counts", () => {
    render(<QualityDashboard summary={SUMMARY} issues={ISSUES} />);
    const chips = screen.getByTestId("quality-severity-chips");
    for (const [sev, count] of Object.entries(SUMMARY.by_severity)) {
      const chip = within(chips).getByTestId(`quality-severity-chip-${sev}`);
      expect(within(chip).getByText(String(count))).toBeInTheDocument();
      expect(within(chip).getByText(sev)).toBeInTheDocument();
    }
  });

  it("marks chips as role=tab and aria-selected=false by default", () => {
    render(<QualityDashboard summary={SUMMARY} issues={ISSUES} />);
    const tablist = screen.getByRole("tablist");
    for (const sev of Object.keys(SUMMARY.by_severity)) {
      const chip = within(tablist).getByTestId(`quality-severity-chip-${sev}`);
      expect(chip).toHaveAttribute("role", "tab");
      expect(chip).toHaveAttribute("aria-selected", "false");
    }
  });

  it("shows the unfiltered issues list by default", () => {
    render(<QualityDashboard summary={SUMMARY} issues={ISSUES} />);
    for (const issue of ISSUES) {
      expect(
        screen.getByTestId(`quality-dashboard-issue-${issue.id}`),
      ).toBeInTheDocument();
    }
  });

  it("clicking a severity chip filters the issues list to that severity", async () => {
    const user = userEvent.setup();
    render(<QualityDashboard summary={SUMMARY} issues={ISSUES} />);
    await user.click(screen.getByTestId("quality-severity-chip-blocker"));
    expect(
      screen.getByTestId("quality-dashboard-issue-1"),
    ).toBeInTheDocument();
    expect(
      screen.queryByTestId("quality-dashboard-issue-2"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByTestId("quality-dashboard-issue-3"),
    ).not.toBeInTheDocument();
  });

  it("clicking the active chip again clears the filter", async () => {
    const user = userEvent.setup();
    render(<QualityDashboard summary={SUMMARY} issues={ISSUES} />);
    const chip = screen.getByTestId("quality-severity-chip-blocker");
    await user.click(chip);
    expect(chip).toHaveAttribute("aria-selected", "true");
    await user.click(chip);
    expect(chip).toHaveAttribute("aria-selected", "false");
    for (const issue of ISSUES) {
      expect(
        screen.getByTestId(`quality-dashboard-issue-${issue.id}`),
      ).toBeInTheDocument();
    }
  });

  it("shows the empty state when the filter matches no issues", async () => {
    const minor: QualitySummaryBlockBody = {
      ...SUMMARY,
      by_severity: { blocker: 0, critical: 0, major: 0, minor: 1, info: 0 },
    };
    const minorIssues: QualityIssueItem[] = [
      {
        id: 99,
        rule_id: "rust:S100",
        severity: "minor",
        category: "naming",
        file: "src/lib.rs",
        line: 99,
        message: "Minor issue",
        status: "open",
        object_id: "issue:99",
      },
    ];
    const user = userEvent.setup();
    render(
      <QualityDashboard summary={minor} issues={minorIssues} />,
    );
    // Filter to critical — none.
    await user.click(screen.getByTestId("quality-severity-chip-critical"));
    expect(screen.getByTestId("quality-dashboard-empty")).toBeInTheDocument();
  });

  it("shows the filtered count + total in the header", async () => {
    const user = userEvent.setup();
    render(<QualityDashboard summary={SUMMARY} issues={ISSUES} />);
    await user.click(screen.getByTestId("quality-severity-chip-blocker"));
    const counter = screen.getByTestId("quality-dashboard-issues-count");
    expect(counter).toHaveTextContent(/1 of 3/);
    expect(counter).toHaveTextContent(/filtered by/);
  });
});
