/**
 * CogniCode probe client (CP0 WU3/WU4/WU8/WU10/WU11).
 *
 * The Backstage backend plugin talks to the Rust service over HTTP. It holds NO
 * canonical CogniCode state: no Findings, no Evidence, no Investigation, no
 * approval or promotion state. Restarting it changes nothing.
 *
 * Identity crossing the boundary is CONTEXT, never authority: the caller's
 * subject is forwarded as a diagnostic header, and the probe response reports
 * `authority: none`.
 */
export const PROBE_PATH = '/control-plane/probe';
export const ACTOR_SUBJECT_HEADER = 'x-cognicode-actor-subject';
export const ACTOR_SOURCE_HEADER = 'x-cognicode-actor-source';

export interface ActorContext {
  subject: string;
  source: 'backstage';
}

export interface CogniCodeProbe {
  service: string;
  service_version: string;
  workspace_id: string | null;
  analysis_identity: string | null;
  symbol_count: number | null;
  relation_count: number | null;
  capabilities: string[];
  actor_context?: {
    actor_subject: string | null;
    actor_source: string | null;
    authority: string;
  };
}

export interface ProbeDeps {
  baseUrl: string;
  actor?: ActorContext;
  fetchImpl: typeof fetch;
  timeoutMs?: number;
}

export class CogniCodeUnavailableError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'CogniCodeUnavailableError';
  }
}

/**
 * Read the probe.
 *
 * Failure is reported as `CogniCodeUnavailableError` — never as stale or
 * fabricated data presented as current truth (WU11).
 */
export async function fetchProbe(deps: ProbeDeps): Promise<CogniCodeProbe> {
  const url = `${deps.baseUrl.replace(/\/+$/, '')}${PROBE_PATH}`;
  const headers: Record<string, string> = { accept: 'application/json' };
  if (deps.actor) {
    headers[ACTOR_SUBJECT_HEADER] = deps.actor.subject;
    headers[ACTOR_SOURCE_HEADER] = deps.actor.source;
  }

  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), deps.timeoutMs ?? 5000);

  let response: Response;
  try {
    response = await deps.fetchImpl(url, { method: 'GET', headers, signal: controller.signal });
  } catch (error) {
    throw new CogniCodeUnavailableError(
      `CogniCode unavailable: ${error instanceof Error ? error.message : String(error)}`,
    );
  } finally {
    clearTimeout(timeout);
  }

  if (!response.ok) {
    throw new CogniCodeUnavailableError(`CogniCode unavailable: HTTP ${response.status}`);
  }

  let body: unknown;
  try {
    body = await response.json();
  } catch {
    throw new CogniCodeUnavailableError('CogniCode unavailable: malformed probe response');
  }
  const probe = body as Partial<CogniCodeProbe>;
  if (typeof probe.service !== 'string' || typeof probe.service_version !== 'string') {
    throw new CogniCodeUnavailableError('CogniCode unavailable: probe contract not satisfied');
  }
  return probe as CogniCodeProbe;
}
