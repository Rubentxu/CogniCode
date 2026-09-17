/**
 * ContextCapsule v1 — the Backstage → Explorer handoff contract (CP0 WU6).
 *
 * Properties that matter:
 *   - BOUNDED: a hard cap on the number of focus refs and on each ref's length.
 *   - VERSIONED: the version is explicit and unknown versions are rejected.
 *   - REFERENCES ONLY: it carries identifiers, never copied Facts/Evidence or
 *     any canonical payload. CogniCode remains the source of truth.
 *
 * There is deliberately NO authority here: a capsule cannot express approval,
 * promotion or authorship. It is navigation context.
 */
export const CONTEXT_CAPSULE_VERSION = 1 as const;
export const MAX_FOCUS_REFS = 32;
export const MAX_REF_LENGTH = 512;

export const CONTEXT_CAPSULE_ORIGINS = ['backstage'] as const;
export type ContextCapsuleOrigin = (typeof CONTEXT_CAPSULE_ORIGINS)[number];

export interface ContextCapsule {
  version: number;
  workspace_id: string;
  snapshot_id?: string;
  focus: string[];
  origin: ContextCapsuleOrigin;
}

export class ContextCapsuleError extends Error {}

export interface BuildContextCapsuleInput {
  workspace_id: string;
  snapshot_id?: string;
  focus?: string[];
  origin?: ContextCapsuleOrigin;
}

/** Build a canonical capsule: validated, bounded, and deterministically ordered. */
export function buildContextCapsule(input: BuildContextCapsuleInput): ContextCapsule {
  const workspaceId = (input.workspace_id ?? '').trim();
  if (!workspaceId) {
    throw new ContextCapsuleError('workspace_id must not be empty');
  }
  const focus = (input.focus ?? []).map(f => f.trim());
  if (focus.some(f => !f)) {
    throw new ContextCapsuleError('focus refs must not be empty');
  }
  const tooLong = focus.find(f => f.length > MAX_REF_LENGTH);
  if (tooLong !== undefined) {
    throw new ContextCapsuleError(`focus ref exceeds ${MAX_REF_LENGTH} chars`);
  }
  const unique = Array.from(new Set(focus)).sort();
  if (unique.length > MAX_FOCUS_REFS) {
    throw new ContextCapsuleError(`too many focus refs (max ${MAX_FOCUS_REFS})`);
  }
  const capsule: ContextCapsule = {
    version: CONTEXT_CAPSULE_VERSION,
    workspace_id: workspaceId,
    focus: unique,
    origin: input.origin ?? 'backstage',
  };
  const snapshotId = (input.snapshot_id ?? '').trim();
  if (snapshotId) {
    capsule.snapshot_id = snapshotId;
  }
  return capsule;
}

/** Encode for a URL. base64url, no padding. */
export function encodeContextCapsule(capsule: ContextCapsule): string {
  return Buffer.from(JSON.stringify(capsule), 'utf8')
    .toString('base64')
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=+$/, '');
}

/** Decode and validate. Unknown versions are rejected, never guessed. */
export function decodeContextCapsule(encoded: string): ContextCapsule {
  const padded = encoded.replace(/-/g, '+').replace(/_/g, '/');
  let parsed: unknown;
  try {
    parsed = JSON.parse(Buffer.from(padded, 'base64').toString('utf8'));
  } catch {
    throw new ContextCapsuleError('capsule is not valid base64url JSON');
  }
  const c = parsed as Partial<ContextCapsule>;
  if (c.version !== CONTEXT_CAPSULE_VERSION) {
    throw new ContextCapsuleError(`unsupported capsule version: ${String(c.version)}`);
  }
  if (typeof c.workspace_id !== 'string' || !c.workspace_id.trim()) {
    throw new ContextCapsuleError('capsule workspace_id must be a non-empty string');
  }
  if (!Array.isArray(c.focus) || c.focus.some(f => typeof f !== 'string')) {
    throw new ContextCapsuleError('capsule focus must be a string array');
  }
  return c as ContextCapsule;
}

/**
 * Deep-link to the Explorer carrying the capsule.
 *
 * No iframe and no duplication of the Explorer UI inside the host: the host
 * hands off, the Explorer owns the inner loop.
 */
export function explorerDeepLink(explorerBaseUrl: string, capsule: ContextCapsule): string {
  const base = explorerBaseUrl.replace(/\/+$/, '');
  return `${base}/?capsule=${encodeContextCapsule(capsule)}`;
}
