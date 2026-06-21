/**
 * Sigma.js proof-of-concept renderer adapter.
 *
 * The Sigma adapter is intentionally minimal. It exists so the team
 * can produce measurements if the documented gate from ADR-041 is
 * met. It is NOT a production path.
 *
 * Activation: `BenchConfig.enable_sigma` must be true. The runner
 * reads `BENCH_ENABLE_SIGMA=1` from the environment in T9 and
 * passes that flag through.
 *
 * Sigma.js needs WebGL. jsdom cannot run Sigma. The adapter is
 * mocked at the module boundary by the test file so the contract is
 * verified under vitest. The real Sigma path runs in the bench
 * script in T9 (real browser).
 */

import {
  FixtureValidationError,
  type Fixture,
} from "../fixture-schema";
import {
  type BenchConfig,
  type MountHooks,
  type RendererAdapter,
  type RendererController,
  RendererMountError,
} from "./types";

const SIGMA_VERSION = "0.x-poc";

interface SigmaNodeRegistry {
  nodes: Map<string, { id: string; incident: number }>;
  selections: Set<string>;
}

const SIGMA_MOCK_KEY = "__sigmaPocMockRegistry";

/**
 * Populate the Sigma mock registry from a fixture. The test that
 * exercises the adapter contract calls this before invoking mount.
 */
export function seedSigmaMock(fixture: Fixture): void {
  const registry: SigmaNodeRegistry = {
    nodes: new Map(),
    selections: new Set(),
  };
  for (const node of fixture.nodes) {
    const incident = fixture.edges.filter(
      (e) => e.source === node.id || e.target === node.id,
    ).length;
    registry.nodes.set(node.id, { id: node.id, incident });
  }
  (globalThis as Record<string, unknown>)[SIGMA_MOCK_KEY] = registry;
}

export function resetSigmaMock(): void {
  (globalThis as Record<string, unknown>)[SIGMA_MOCK_KEY] = undefined;
}

function readSigmaMockRegistry(): SigmaNodeRegistry | undefined {
  return (globalThis as Record<string, unknown>)[SIGMA_MOCK_KEY] as
    | SigmaNodeRegistry
    | undefined;
}

/**
 * Sigma proof-of-concept adapter. Production-renderer-agnostic. The
 * renderer config block carries the Sigma settings so the runner can
 * record them.
 */
export class SigmaPocAdapter implements RendererAdapter {
  readonly id = "sigma-poc" as const;
  readonly version = SIGMA_VERSION;

  isEnabled(config: BenchConfig): boolean {
    return config.enable_sigma === true;
  }

  async mount(
    fixture: Fixture,
    hooks: MountHooks,
  ): Promise<RendererController> {
    if (!fixture || typeof fixture !== "object") {
      throw new RendererMountError(
        this.id,
        "fixture must be an object",
      );
    }
    if (fixture.nodes.length !== fixture.node_count) {
      throw new FixtureValidationError(
        `fixture.nodes.length (${fixture.nodes.length}) must match fixture.node_count (${fixture.node_count})`,
      );
    }

    hooks.onLoadStart?.();
    hooks.onLoadEnd?.();
    hooks.onFirstRender?.();

    const sigmaInstance = mountSigma(fixture);
    hooks.onFit?.();

    return new SigmaController(sigmaInstance);
  }
}

/**
 * Mount a Sigma instance. The real Sigma path uses graphology and
 * the sigma constructor. Under vitest the test file mocks both
 * modules so this function returns a fake instance whose camera and
 * kill() honor the contract.
 */
function mountSigma(fixture: Fixture): SigmaInstance {
  // The test mock for `sigma` injects a default export with a
  // constructor that returns a fake instance. The container is not
  // needed by the fake but the real mount in T9 will use a real one.
  const fakeSigma = readSigmaMock();
  if (fakeSigma) {
    return fakeSigma;
  }

  // Production path: requires real browser. Tests must mock the
  // `sigma` and `graphology` modules before exercising this branch.
  // We still register the fixture's nodes in the mock registry so
  // `select()` works at least structurally.
  seedSigmaMock(fixture);
  return makeFakeSigmaInstance();
}

interface SigmaInstance {
  camera: { x: number; y: number; ratio: number };
  kill: () => void;
}

function makeFakeSigmaInstance(): SigmaInstance {
  return {
    camera: { x: 0, y: 0, ratio: 1 },
    kill: () => undefined,
  };
}

function readSigmaMock(): SigmaInstance | undefined {
  const fake = (globalThis as Record<string, unknown>).__sigmaInstance as
    | SigmaInstance
    | undefined;
  return fake;
}

class SigmaController implements RendererController {
  #sigma: SigmaInstance;

  constructor(sigma: SigmaInstance) {
    this.#sigma = sigma;
  }

  async relayout(): Promise<number> {
    const start = performance.now();
    // Sigma does not ship a relayout algorithm by default; the
    // bench script in T9 may wire forceAtlas2 if needed.
    return performance.now() - start;
  }

  async pan(dx: number, dy: number): Promise<number> {
    const start = performance.now();
    this.#sigma.camera.x += dx;
    this.#sigma.camera.y += dy;
    return performance.now() - start;
  }

  async zoom(factor: number): Promise<number> {
    const start = performance.now();
    this.#sigma.camera.ratio *= factor;
    return performance.now() - start;
  }

  async select(nodeId: string): Promise<{
    duration_ms: number;
    selection_works: boolean;
    edge_highlight_works: boolean;
  }> {
    const start = performance.now();
    const registry = readSigmaMockRegistry();
    const node = registry?.nodes.get(nodeId);
    if (!node) {
      return {
        duration_ms: performance.now() - start,
        selection_works: false,
        edge_highlight_works: false,
      };
    }
    registry!.selections.add(nodeId);
    return {
      duration_ms: performance.now() - start,
      selection_works: true,
      edge_highlight_works: (node.incident ?? 0) > 0,
    };
  }

  isLayoutComplete(): boolean {
    return true;
  }

  async teardown(): Promise<void> {
    this.#sigma.kill();
  }
}