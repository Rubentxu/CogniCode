/**
 * Shared cytoscape mock state for the `ContextualPanel` test suite.
 *
 * `NeighborMinigraph` mounts a cytoscape instance. Each test file
 * that exercises that component defines its own `vi.mock("cytoscape")`
 * factory inline (vitest hoists vi.mock only at the top of the test
 * file, so the mock factory must be a sibling of the imports it
 * needs to mock). The factory writes every constructed instance
 * into `globalThis.__cyInstances`; this helper exposes
 * `resetCyMock` + `getCyInstances` for `beforeEach` cleanup and
 * assertions.
 *
 * This file deliberately does NOT call `vi.mock` itself — only the
 * `CyMock` type + store accessors.
 */

export type CyMock = {
  nodes: { id: string; data: { id: string } }[];
  edges: { id: string; data: { id: string; source: string; target: string } }[];
  destroyed: boolean;
  clickNode: (id: string) => void;
};

declare global {
  var __cyInstances: CyMock[] | undefined;
}
globalThis.__cyInstances = [];

export function resetCyMock() {
  globalThis.__cyInstances = [];
}

export function getCyInstances(): CyMock[] {
  return globalThis.__cyInstances ?? [];
}
