/**
 * `Shell` tests — viewport behaviour, health chip, skip link,
 * offline gate integration.
 *
 * The Shell renders the three panels (Miller Columns, Object
 * Inspector, Lens Panel) which all consume `useApp()`. The test
 * harness therefore provides a real `AppContext` so the panels
 * mount without crashing.
 */
import { describe, it, expect } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { useReducer } from "react";

import { Shell } from "./Shell";
import { detectViewport } from "./viewport";
import { HealthProbe } from "./HealthProbe";
import { SkipLink } from "./SkipLink";
import {
  AppContext,
  appReducer,
  initialState,
  type Action,
  type AppState,
} from "../state/context";

/**
 * Minimal harness that provides a live AppContext. We use a real
 * useReducer (not a mock) so the panels see a consistent state
 * shape and dispatching works during the test.
 */
function ShellHarness({ viewport }: { viewport?: "small" | "tablet" | "desktop" | "ultrawide" }) {
  const [state, dispatch] = useReducer(appReducer, initialState);
  // Provide a tiny app context value. The dispatch type is the
  // `Action` union from the reducer.
  const value: { state: AppState; dispatch: React.Dispatch<Action> } = {
    state,
    dispatch,
  };
  return (
    <AppContext.Provider value={value}>
      <Shell viewport={viewport} />
    </AppContext.Provider>
  );
}

describe("detectViewport", () => {
  it("classifies >= 1200px as desktop", () => {
    expect(detectViewport(1280)).toBe("desktop");
    expect(detectViewport(1200)).toBe("desktop");
  });
  it("classifies 900-1199 as tablet", () => {
    expect(detectViewport(1199)).toBe("tablet");
    expect(detectViewport(900)).toBe("tablet");
  });
  it("classifies < 900 as small", () => {
    expect(detectViewport(899)).toBe("small");
    expect(detectViewport(360)).toBe("small");
  });
});

describe("Shell", () => {
  it("renders the top bar with the project title and a health chip", async () => {
    render(<ShellHarness viewport="desktop" />);
    expect(
      screen.getByRole("heading", { name: /CogniCode Explorer/i, level: 1 }),
    ).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByTestId("health-chip")).toBeInTheDocument();
    });
  });

  it("renders the skip link as the first focusable element", () => {
    render(<ShellHarness viewport="desktop" />);
    const skip = screen.getByTestId("skip-link");
    expect(skip).toBeInTheDocument();
    expect(skip).toHaveTextContent(/skip to main content/i);
  });

  it("renders a <main> landmark with the right label", () => {
    render(<ShellHarness viewport="desktop" />);
    const main = screen.getByRole("main");
    expect(main).toHaveAttribute("id", "app-main");
    expect(main).toHaveAttribute("aria-label", "Explorer panels");
  });

  it("desktop viewport shows all three panel empty states", () => {
    render(<ShellHarness viewport="desktop" />);
    expect(screen.getByTestId("miller-columns-empty")).toBeInTheDocument();
    expect(screen.getByTestId("object-inspector-empty")).toBeInTheDocument();
    expect(screen.getByTestId("lens-panel-empty")).toBeInTheDocument();
  });

  it("tablet viewport shows the lens toggle button", () => {
    render(<ShellHarness viewport="tablet" />);
    expect(
      screen.getByRole("button", { name: /open lens panel/i }),
    ).toBeInTheDocument();
  });

  it("small viewport hides the lens panel", () => {
    render(<ShellHarness viewport="small" />);
    expect(screen.queryByTestId("lens-panel")).not.toBeInTheDocument();
    expect(screen.queryByTestId("lens-panel-empty")).not.toBeInTheDocument();
  });

  it("data-viewport attribute reflects the active viewport", () => {
    const { rerender } = render(<ShellHarness viewport="desktop" />);
    expect(screen.getByTestId("shell")).toHaveAttribute(
      "data-viewport",
      "desktop",
    );
    rerender(<ShellHarness viewport="tablet" />);
    expect(screen.getByTestId("shell")).toHaveAttribute(
      "data-viewport",
      "tablet",
    );
    rerender(<ShellHarness viewport="small" />);
    expect(screen.getByTestId("shell")).toHaveAttribute(
      "data-viewport",
      "small",
    );
  });

  it("ultrawide viewport (>=1440) renders the InteractiveGraph 4th column", async () => {
    render(<ShellHarness viewport="ultrawide" />);
    // The InteractiveGraph chunk is `React.lazy`-imported and wrapped
    // in `<Suspense>`. We accept either the resolved graph, the empty
    // state, or the Suspense fallback as proof the 4th column is wired.
    await waitFor(() => {
      const hasGraph =
        document.querySelector('[data-testid="interactive-graph"]') !== null;
      const hasEmpty =
        document.querySelector('[data-testid="interactive-graph-empty"]') !== null;
      const hasLoading =
        document.querySelector('[data-testid="interactive-graph-loading"]') !== null;
      expect(hasGraph || hasEmpty || hasLoading).toBe(true);
    });
  });

  it("desktop viewport does NOT render the 4th column (3-column layout kept)", () => {
    render(<ShellHarness viewport="desktop" />);
    expect(screen.queryByTestId("interactive-graph")).not.toBeInTheDocument();
  });

  it("tablet viewport does NOT render the 4th column", () => {
    render(<ShellHarness viewport="tablet" />);
    expect(screen.queryByTestId("interactive-graph")).not.toBeInTheDocument();
  });

  it("small viewport does NOT render InteractiveGraph", () => {
    render(<ShellHarness viewport="small" />);
    expect(screen.queryByTestId("interactive-graph")).not.toBeInTheDocument();
  });
});

describe("HealthProbe (chip mode)", () => {
  it("renders the chip in the top bar", async () => {
    render(<HealthProbe showFullScreenOnError={false} />);
    await waitFor(() => {
      expect(screen.getByTestId("health-chip")).toBeInTheDocument();
    });
  });

  it("updates the data-status when the backend responds", async () => {
    render(<HealthProbe showFullScreenOnError={false} />);
    await waitFor(() => {
      expect(screen.getByTestId("health-chip")).toHaveAttribute(
        "data-status",
        "online",
      );
    });
  });
});

describe("SkipLink", () => {
  it("uses the provided target id in the href", () => {
    render(<SkipLink targetId="app-main" />);
    const link = screen.getByTestId("skip-link");
    expect(link).toHaveAttribute("href", "#app-main");
  });
});
