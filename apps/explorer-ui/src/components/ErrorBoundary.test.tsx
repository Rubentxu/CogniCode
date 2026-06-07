import { describe, it, expect, vi } from "vitest";
import { useState } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ErrorBoundary, DefaultErrorFallback } from "./ErrorBoundary";

function Bomb({ shouldThrow }: { shouldThrow: boolean }) {
  if (shouldThrow) {
    throw new Error("kaboom");
  }
  return <span>all good</span>;
}

function RecoverableHarness() {
  const [shouldThrow, setShouldThrow] = useState(true);
  return (
    <ErrorBoundary label="Graph" onReset={() => setShouldThrow(false)}>
      <Bomb shouldThrow={shouldThrow} />
    </ErrorBoundary>
  );
}

describe("ErrorBoundary", () => {
  // Suppress noisy React error logs during expected throws.
  const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});

  it("renders children when no error is thrown", () => {
    render(
      <ErrorBoundary label="test">
        <Bomb shouldThrow={false} />
      </ErrorBoundary>,
    );
    expect(screen.getByText("all good")).toBeInTheDocument();
  });

  it("renders default fallback when a child throws", () => {
    render(
      <ErrorBoundary label="MillerColumns">
        <Bomb shouldThrow={true} />
      </ErrorBoundary>,
    );
    expect(screen.getByRole("alert")).toHaveTextContent(/MillerColumns crashed/i);
    expect(screen.getByRole("alert")).toHaveTextContent("kaboom");
  });

  it("renders custom fallback when provided", () => {
    render(
      <ErrorBoundary
        label="ObjectInspector"
        fallback={(error, reset) => (
          <div>
            <span>Custom: {error.message}</span>
            <button onClick={reset}>Reset</button>
          </div>
        )}
      >
        <Bomb shouldThrow={true} />
      </ErrorBoundary>,
    );
    expect(screen.getByText("Custom: kaboom")).toBeInTheDocument();
  });

  it("calls onReset and recovers when Retry is clicked", async () => {
    const user = userEvent.setup();
    render(<RecoverableHarness />);
    // Initially the bomb throws and the boundary shows the fallback.
    expect(screen.getByText("kaboom")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /retry/i }));
    // onReset updates parent state (shouldThrow=false), and the boundary
    // resets its own error state — the children render cleanly on next paint.
    expect(await screen.findByText("all good")).toBeInTheDocument();
  });

  it("DefaultErrorFallback has an alert role and accessible label", () => {
    const error = new Error("unit-test error");
    render(
      <DefaultErrorFallback
        error={error}
        onReset={() => {}}
        label="LensPanel"
      />,
    );
    const alert = screen.getByRole("alert");
    expect(alert).toHaveAttribute("aria-live", "assertive");
    expect(alert).toHaveTextContent("LensPanel crashed");
    expect(alert).toHaveTextContent("unit-test error");
  });

  it("does not log twice: caught error goes through componentDidCatch once", () => {
    // Reset and re-arm the spy so we count this test in isolation.
    consoleError.mockClear();
    render(
      <ErrorBoundary label="counter">
        <Bomb shouldThrow={true} />
      </ErrorBoundary>,
    );
    // One console.error for the React rendering error, one for our own
    // componentDidCatch surfacing — we want our surfacing to be exactly 1.
    const ourCalls = consoleError.mock.calls.filter((args) =>
      String(args[0] ?? "").includes("[ErrorBoundary:counter]"),
    );
    expect(ourCalls.length).toBe(1);
  });
});
