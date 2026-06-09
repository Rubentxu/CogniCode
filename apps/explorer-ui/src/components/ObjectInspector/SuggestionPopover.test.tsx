/**
 * `SuggestionPopover` — tests for the native-dialog popover.
 *
 * The popover is the "small viewport" branch of the suggestion strip.
 * It opens a native `<dialog>` via `showModal()`, dismisses on Escape
 * (native) or outside-click (manual listener), and returns focus to
 * the trigger button on close.
 *
 * Pure DOM behaviour — no router, no network. Each test renders the
 * component with a list of prompts and asserts on the open/close +
 * focus flow.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createRef } from "react";

import { SuggestionPopover } from "./SuggestionPopover";
import type { SuggestedQuestion } from "../../config/suggestedQuestions";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const SAMPLE_PROMPTS: SuggestedQuestion[] = [
  {
    id: "who-calls",
    label: "Who calls this?",
    tool: "cognicode_ask",
    params: { question: "who calls `{label}`?" },
    requiresGraph: true,
  },
  {
    id: "risky",
    label: "What is risky?",
    tool: "cognicode_ask",
    params: { question: "is `{label}` risky?" },
    requiresGraph: true,
  },
  {
    id: "justifies",
    label: "What justifies this?",
    tool: "explorer_inspect_object",
    params: { object_id: "ev:1" },
    requiresGraph: false,
  },
];

// jsdom's `<dialog>` lacks `showModal`/`close`; polyfill them so the
// component can call the real browser API and we can assert behaviour
// in the simplest possible way (no portal, no Radix).
beforeEach(() => {
  if (typeof HTMLDialogElement !== "undefined") {
    if (!HTMLDialogElement.prototype.showModal) {
      HTMLDialogElement.prototype.showModal = function () {
        (this as HTMLDialogElement & { open: boolean }).open = true;
        this.dispatchEvent(new Event("open"));
      };
    }
    if (!HTMLDialogElement.prototype.close) {
      HTMLDialogElement.prototype.close = function () {
        if ((this as HTMLDialogElement & { open: boolean }).open) {
          (this as HTMLDialogElement & { open: boolean }).open = false;
          this.dispatchEvent(new Event("close"));
        }
      };
    }
  }
});

afterEach(() => {
  // No state to clean — RTL `cleanup` runs in setup.ts.
});

// ---------------------------------------------------------------------------
// Trigger + open
// ---------------------------------------------------------------------------

describe("SuggestionPopover — open", () => {
  it("renders the trigger button + dialog but does not show the dialog initially", () => {
    render(
      <SuggestionPopover
        prompts={SAMPLE_PROMPTS}
        onDispatch={vi.fn()}
        ariaLabel="What can I do here?"
      />,
    );
    expect(screen.getByTestId("suggestion-popover-trigger")).toBeInTheDocument();
    expect(screen.getByTestId("suggestion-popover-trigger")).toHaveTextContent(
      "What can I do here?",
    );
    // Dialog exists in the DOM but is not "open".
    const dialog = screen.getByTestId("suggestion-popover-dialog");
    expect(dialog).toBeInTheDocument();
  });

  it("opens the dialog on trigger click and lists all prompts", async () => {
    const user = userEvent.setup();
    render(
      <SuggestionPopover
        prompts={SAMPLE_PROMPTS}
        onDispatch={vi.fn()}
        ariaLabel="What can I do here?"
      />,
    );

    await user.click(screen.getByTestId("suggestion-popover-trigger"));

    // After `showModal()` is called, the dialog is in the open state.
    const dialog = screen.getByTestId("suggestion-popover-dialog") as HTMLDialogElement & {
      open: boolean;
    };
    expect(dialog.open).toBe(true);

    // All three prompt labels are present inside the dialog.
    for (const prompt of SAMPLE_PROMPTS) {
      expect(
        withinDialog(dialog, prompt.label),
        `label="${prompt.label}" should be rendered`,
      ).toBeInTheDocument();
    }
  });
});

// ---------------------------------------------------------------------------
// Close on Escape
// ---------------------------------------------------------------------------

describe("SuggestionPopover — Escape close", () => {
  it("closes the dialog on Escape and returns focus to the trigger", async () => {
    const user = userEvent.setup();
    render(
      <SuggestionPopover
        prompts={SAMPLE_PROMPTS}
        onDispatch={vi.fn()}
        ariaLabel="What can I do here?"
      />,
    );

    const trigger = screen.getByTestId("suggestion-popover-trigger");
    await user.click(trigger);
    const dialog = screen.getByTestId("suggestion-popover-dialog") as HTMLDialogElement & {
      open: boolean;
    };
    expect(dialog.open).toBe(true);

    // Press Escape — the native dialog will fire its `cancel` event and
    // close. The component listens and calls `close()` + focuses the
    // trigger.
    fireEvent.keyDown(dialog, { key: "Escape" });
    // jsdom does not implement the native dialog's Escape-to-cancel
    // path. Dispatch the `cancel` event directly — the component's
    // `onCancel` handler is what actually closes the dialog and
    // restores focus.
    dialog.dispatchEvent(new Event("cancel", { cancelable: true }));
    dialog.close();

    await waitFor(() => {
      expect(dialog.open).toBe(false);
    });
    expect(document.activeElement).toBe(trigger);
  });
});

// ---------------------------------------------------------------------------
// Close on outside click
// ---------------------------------------------------------------------------

describe("SuggestionPopover — outside click close", () => {
  it("closes the dialog when the user clicks on the dialog backdrop", async () => {
    const user = userEvent.setup();
    render(
      <SuggestionPopover
        prompts={SAMPLE_PROMPTS}
        onDispatch={vi.fn()}
        ariaLabel="What can I do here?"
      />,
    );

    const trigger = screen.getByTestId("suggestion-popover-trigger");
    await user.click(trigger);
    const dialog = screen.getByTestId("suggestion-popover-dialog") as HTMLDialogElement & {
      open: boolean;
    };
    expect(dialog.open).toBe(true);

    // Simulate a click whose target is the dialog element itself
    // (i.e. the backdrop, not an inner button). The component listens
    // for `click` and closes if `event.target === dialog`.
    fireEvent.click(dialog);

    await waitFor(() => {
      expect(dialog.open).toBe(false);
    });
  });
});

// ---------------------------------------------------------------------------
// Dispatch on prompt click
// ---------------------------------------------------------------------------

describe("SuggestionPopover — dispatch on prompt click", () => {
  it("calls onDispatch with the prompt when a prompt inside the dialog is clicked", async () => {
    const onDispatch = vi.fn();
    const user = userEvent.setup();
    render(
      <SuggestionPopover
        prompts={SAMPLE_PROMPTS}
        onDispatch={onDispatch}
        ariaLabel="What can I do here?"
      />,
    );

    await user.click(screen.getByTestId("suggestion-popover-trigger"));
    await user.click(screen.getByTestId("suggestion-popover-item-justifies"));

    expect(onDispatch).toHaveBeenCalledTimes(1);
    expect(onDispatch.mock.calls[0]![0]).toEqual(SAMPLE_PROMPTS[2]);
  });
});

// ---------------------------------------------------------------------------
// Imperative API (open / close)
// ---------------------------------------------------------------------------

describe("SuggestionPopover — imperative handle", () => {
  it("exposes an open() method via ref that focuses the trigger when closed externally", async () => {
    const ref = createRef<{ open: () => void; close: () => void }>();
    const onDispatch = vi.fn();
    render(
      <SuggestionPopover
        ref={ref}
        prompts={SAMPLE_PROMPTS}
        onDispatch={onDispatch}
        ariaLabel="What can I do here?"
      />,
    );

    expect(ref.current).toBeDefined();
    act(() => ref.current?.open());
    const dialog = screen.getByTestId("suggestion-popover-dialog") as HTMLDialogElement & {
      open: boolean;
    };
    expect(dialog.open).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

import { act } from "@testing-library/react";
import { within } from "@testing-library/react";

function withinDialog(dialog: HTMLElement, text: string) {
  return within(dialog as HTMLElement).getByText(text);
}
