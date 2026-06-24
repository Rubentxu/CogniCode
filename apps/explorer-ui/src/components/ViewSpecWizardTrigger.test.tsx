/**
 * Component tests for ViewSpecWizardTrigger — the header button
 * that opens the ViewSpecWizard modal.
 *
 * Covers:
 * - Renders with correct aria-label/aria-pressed
 * - Disabled when no object is active
 * - Enabled when an object is active
 * - Click dispatches TOGGLE_VIEWSPEC_WIZARD
 * - Visual state reflects viewSpecWizard.open from global state
 */

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

// Mock the state hooks so we can drive them from tests.
const mockDispatch = vi.fn();
let mockState: {
  activeObjectId: string | null;
  viewSpecWizard: { open: boolean };
} = {
  activeObjectId: "sym:abc",
  viewSpecWizard: { open: false },
};

vi.mock("../state/context", () => ({
  useAppDispatch: () => mockDispatch,
  useAppState: () => mockState,
}));

import { ViewSpecWizardTrigger } from "./ViewSpecWizardTrigger";

describe("ViewSpecWizardTrigger", () => {
  it("renders an enabled button with the open label when no object is selected", () => {
    // (cannot be — by default activeObjectId is null when disabled).
    // See "is disabled when no object is active" test.
    mockState = {
      activeObjectId: "sym:abc",
      viewSpecWizard: { open: false },
    };
    render(<ViewSpecWizardTrigger />);
    const btn = screen.getByTestId("viewspec-wizard-trigger");
    expect(btn).toBeInTheDocument();
    expect(btn).not.toBeDisabled();
    expect(btn.getAttribute("aria-label")).toBe("Create custom view");
    expect(btn.getAttribute("aria-pressed")).toBe("false");
  });

  it("is disabled when no object is active", () => {
    mockState = {
      activeObjectId: null,
      viewSpecWizard: { open: false },
    };
    render(<ViewSpecWizardTrigger />);
    const btn = screen.getByTestId("viewspec-wizard-trigger");
    expect(btn).toBeDisabled();
    expect(btn.getAttribute("aria-disabled")).toBe("true");
    expect(btn.getAttribute("title")).toBe(
      "Select an object first to create a custom view",
    );
  });

  it("does not dispatch when clicked while disabled", () => {
    mockState = {
      activeObjectId: null,
      viewSpecWizard: { open: false },
    };
    render(<ViewSpecWizardTrigger />);
    fireEvent.click(screen.getByTestId("viewspec-wizard-trigger"));
    expect(mockDispatch).not.toHaveBeenCalled();
  });

  it("dispatches TOGGLE_VIEWSPEC_WIZARD on click when an object is active", () => {
    mockState = {
      activeObjectId: "sym:abc",
      viewSpecWizard: { open: false },
    };
    render(<ViewSpecWizardTrigger />);
    fireEvent.click(screen.getByTestId("viewspec-wizard-trigger"));
    expect(mockDispatch).toHaveBeenCalledWith({
      type: "TOGGLE_VIEWSPEC_WIZARD",
    });
  });

  it("reflects the wizard open state via aria-pressed", () => {
    mockState = {
      activeObjectId: "sym:abc",
      viewSpecWizard: { open: true },
    };
    render(<ViewSpecWizardTrigger />);
    const btn = screen.getByTestId("viewspec-wizard-trigger");
    expect(btn.getAttribute("aria-pressed")).toBe("true");
    expect(btn.getAttribute("aria-label")).toBe("Close custom view wizard");
    expect(btn.getAttribute("title")).toBe("Close custom view wizard");
  });

  it("reflects the wizard closed state via aria-pressed", () => {
    mockState = {
      activeObjectId: "sym:abc",
      viewSpecWizard: { open: false },
    };
    render(<ViewSpecWizardTrigger />);
    const btn = screen.getByTestId("viewspec-wizard-trigger");
    expect(btn.getAttribute("aria-pressed")).toBe("false");
    expect(btn.getAttribute("aria-label")).toBe("Create custom view");
  });
});
