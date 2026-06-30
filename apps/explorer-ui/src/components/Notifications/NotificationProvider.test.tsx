/**
 * Unit tests for NotificationProvider component.
 *
 * Verifies: toast rendering, auto-dismiss after 6 seconds,
 * notification context usage.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { useContext } from "react";

import { NotificationProvider, NotificationContext } from "./NotificationProvider";

// Test component that uses the notification context
function TestPublisher() {
  const { showNotification } = useContext(NotificationContext);

  return (
    <div>
      <button
        data-testid="publish-1"
        onClick={() => showNotification("First toast")}
      >
        First
      </button>
      <button
        data-testid="publish-2"
        onClick={() => showNotification("Second toast")}
      >
        Second
      </button>
    </div>
  );
}

function ToastTrigger() {
  const { showNotification } = useContext(NotificationContext);
  return (
    <button
      data-testid="show-toast"
      onClick={() => showNotification("Test toast")}
    >
      Show Toast
    </button>
  );
}

describe("NotificationProvider component", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders children without showing toasts initially", () => {
    render(
      <NotificationProvider>
        <div data-testid="child">Child Content</div>
      </NotificationProvider>,
    );

    expect(screen.getByTestId("child")).toBeVisible();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("shows toast when showNotification is called", () => {
    render(
      <NotificationProvider>
        <TestPublisher />
      </NotificationProvider>,
    );

    act(() => {
      screen.getByTestId("publish-1").click();
    });

    expect(screen.getByRole("alert")).toHaveTextContent("First toast");
  });

  it("auto-dismisses after 6 seconds", () => {
    render(
      <NotificationProvider>
        <ToastTrigger />
      </NotificationProvider>,
    );

    // Show the toast
    act(() => {
      screen.getByTestId("show-toast").click();
    });

    // Toast should be visible immediately
    expect(screen.getByRole("alert")).toHaveTextContent("Test toast");

    // Advance time by 5 seconds - should NOT be dismissed yet
    act(() => {
      vi.advanceTimersByTime(5000);
    });
    expect(screen.queryByRole("alert")).toBeInTheDocument();

    // Advance time by another 1 second (total 6 seconds) - should NOW be dismissed
    act(() => {
      vi.advanceTimersByTime(1000);
    });
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("renders multiple toasts", () => {
    render(
      <NotificationProvider>
        <TestPublisher />
      </NotificationProvider>,
    );

    act(() => {
      screen.getByTestId("publish-1").click();
      screen.getByTestId("publish-2").click();
    });

    const alerts = screen.getAllByRole("alert");
    expect(alerts).toHaveLength(2);
    expect(alerts[0]).toHaveTextContent("First toast");
    expect(alerts[1]).toHaveTextContent("Second toast");
  });

  it("each toast dismisses independently after 6 seconds", () => {
    render(
      <NotificationProvider>
        <TestPublisher />
      </NotificationProvider>,
    );

    // Show two toasts
    act(() => {
      screen.getByTestId("publish-1").click();
      screen.getByTestId("publish-2").click();
    });

    expect(screen.getAllByRole("alert")).toHaveLength(2);

    // Advance 6 seconds - both should dismiss
    act(() => {
      vi.advanceTimersByTime(6000);
    });

    expect(screen.queryByRole("alert")).toBeNull();
  });
});
