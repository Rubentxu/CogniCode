/**
 * `Fixture` schema and runtime validator.
 *
 * The E7 benchmark harness consumes fixtures exclusively through this
 * schema. New renderer adapters MUST type their input as `Fixture`.
 *
 * Keeping the schema hand-written (no zod) avoids a new runtime
 * dependency for what is, today, a small contract.
 */

export type FixtureKind =
  | "call_graph"
  | "dependency_graph"
  | "architecture_c4"
  | "landing_overview";

export type SizeBand = "small" | "medium" | "large";

export interface FixtureNode {
  id: string;
  label: string;
  kind: string;
  style_class: string | null;
}

export interface FixtureEdge {
  id: string;
  source: string;
  target: string;
  relation: string;
  style_class: string | null;
}

export interface Fixture {
  fixture_id: string;
  kind: FixtureKind;
  size_band: SizeBand;
  node_count: number;
  edge_count: number;
  nodes: FixtureNode[];
  edges: FixtureEdge[];
}

export class FixtureValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "FixtureValidationError";
  }
}

const ALLOWED_KINDS: ReadonlySet<FixtureKind> = new Set([
  "call_graph",
  "dependency_graph",
  "architecture_c4",
  "landing_overview",
]);

const ALLOWED_BANDS: ReadonlySet<SizeBand> = new Set([
  "small",
  "medium",
  "large",
]);

/**
 * Validate that `value` conforms to the `Fixture` contract.
 * Throws `FixtureValidationError` on the first structural violation.
 */
export function assertFixture(value: unknown): asserts value is Fixture {
  if (!isPlainObject(value)) {
    throw new FixtureValidationError("fixture must be an object");
  }

  const fixture = value as Record<string, unknown>;

  const requiredStrings: Array<keyof Fixture & string> = [
    "fixture_id",
    "kind",
    "size_band",
  ];
  for (const key of requiredStrings) {
    if (typeof fixture[key] !== "string" || fixture[key] === "") {
      throw new FixtureValidationError(
        `fixture.${key} must be a non-empty string`,
      );
    }
  }

  if (!ALLOWED_KINDS.has(fixture.kind as FixtureKind)) {
    throw new FixtureValidationError(
      `fixture.kind must be one of ${[...ALLOWED_KINDS].join(", ")}`,
    );
  }

  if (!ALLOWED_BANDS.has(fixture.size_band as SizeBand)) {
    throw new FixtureValidationError(
      `fixture.size_band must be one of ${[...ALLOWED_BANDS].join(", ")}`,
    );
  }

  if (!isPositiveInteger(fixture.node_count)) {
    throw new FixtureValidationError(
      "fixture.node_count must be a positive integer",
    );
  }

  if (!isNonNegativeInteger(fixture.edge_count)) {
    throw new FixtureValidationError(
      "fixture.edge_count must be a non-negative integer",
    );
  }

  if (!Array.isArray(fixture.nodes)) {
    throw new FixtureValidationError("fixture.nodes must be an array");
  }
  if (fixture.nodes.length !== fixture.node_count) {
    throw new FixtureValidationError(
      `fixture.nodes.length (${fixture.nodes.length}) must match fixture.node_count (${fixture.node_count})`,
    );
  }
  const nodeIds = new Set<string>();
  for (const [i, node] of fixture.nodes.entries()) {
    assertFixtureNode(node, i);
    const id = (node as FixtureNode).id;
    if (nodeIds.has(id)) {
      throw new FixtureValidationError(
        `fixture.nodes[${i}].id duplicates an earlier node id: ${id}`,
      );
    }
    nodeIds.add(id);
  }

  if (!Array.isArray(fixture.edges)) {
    throw new FixtureValidationError("fixture.edges must be an array");
  }
  if (fixture.edges.length !== fixture.edge_count) {
    throw new FixtureValidationError(
      `fixture.edges.length (${fixture.edges.length}) must match fixture.edge_count (${fixture.edge_count})`,
    );
  }
  for (const [i, edge] of fixture.edges.entries()) {
    assertFixtureEdge(edge, i, nodeIds);
  }
}

function assertFixtureNode(value: unknown, index: number): void {
  if (!isPlainObject(value)) {
    throw new FixtureValidationError(
      `fixture.nodes[${index}] must be an object`,
    );
  }
  const node = value as Partial<FixtureNode>;
  if (typeof node.id !== "string" || node.id === "") {
    throw new FixtureValidationError(
      `fixture.nodes[${index}].id must be a non-empty string`,
    );
  }
  if (typeof node.label !== "string") {
    throw new FixtureValidationError(
      `fixture.nodes[${index}].label must be a string`,
    );
  }
  if (typeof node.kind !== "string" || node.kind === "") {
    throw new FixtureValidationError(
      `fixture.nodes[${index}].kind must be a non-empty string`,
    );
  }
  if (node.style_class !== null && typeof node.style_class !== "string") {
    throw new FixtureValidationError(
      `fixture.nodes[${index}].style_class must be a string or null`,
    );
  }
}

function assertFixtureEdge(
  value: unknown,
  index: number,
  nodeIds: ReadonlySet<string>,
): void {
  if (!isPlainObject(value)) {
    throw new FixtureValidationError(
      `fixture.edges[${index}] must be an object`,
    );
  }
  const edge = value as Partial<FixtureEdge>;
  if (typeof edge.id !== "string" || edge.id === "") {
    throw new FixtureValidationError(
      `fixture.edges[${index}].id must be a non-empty string`,
    );
  }
  if (typeof edge.source !== "string" || !nodeIds.has(edge.source)) {
    throw new FixtureValidationError(
      `fixture.edges[${index}].source must reference an existing node id`,
    );
  }
  if (typeof edge.target !== "string" || !nodeIds.has(edge.target)) {
    throw new FixtureValidationError(
      `fixture.edges[${index}].target must reference an existing node id`,
    );
  }
  if (edge.source === edge.target) {
    throw new FixtureValidationError(
      `fixture.edges[${index}] cannot reference the same node as both source and target`,
    );
  }
  if (typeof edge.relation !== "string" || edge.relation === "") {
    throw new FixtureValidationError(
      `fixture.edges[${index}].relation must be a non-empty string`,
    );
  }
  if (edge.style_class !== null && typeof edge.style_class !== "string") {
    throw new FixtureValidationError(
      `fixture.edges[${index}].style_class must be a string or null`,
    );
  }
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return (
    typeof value === "object" &&
    value !== null &&
    !Array.isArray(value) &&
    Object.getPrototypeOf(value) === Object.prototype
  );
}

function isPositiveInteger(value: unknown): boolean {
  return typeof value === "number" && Number.isInteger(value) && value > 0;
}

function isNonNegativeInteger(value: unknown): boolean {
  return typeof value === "number" && Number.isInteger(value) && value >= 0;
}