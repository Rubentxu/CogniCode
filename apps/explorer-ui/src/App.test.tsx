/**
 * Smoke tests for the App root.
 *
 * The root App now mounts a `ConnectionGate` that probes the backend
 * before swapping in the `Shell`. The MSW handler returns a healthy
 * response, so the gate transitions to `online` on the first paint
 * and the Shell renders the top bar with the health chip.
 */
import { describe, it, expect } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

import App from "./App";

describe("App", () => {
  it("renders the Explorer shell with the project title", async () => {
    render(<App />);
    expect(
      await screen.findByRole("heading", { name: /CogniCode Explorer/i, level: 1 }),
    ).toBeInTheDocument();
  });

  it("renders the health chip after the connection probe resolves", async () => {
    render(<App />);
    await waitFor(() => {
      expect(screen.getByTestId("health-chip")).toBeInTheDocument();
    });
  });
});
