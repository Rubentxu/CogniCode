/**
 * Minimal `/cognicode` page (CP0 WU4). Deliberately NOT a dashboard: it proves
 * the shell can show real service state and hand off to the Explorer.
 */
import React, { useEffect, useState } from 'react';
import { buildContextCapsule, explorerDeepLink } from '../../../src/context-capsule';
import type { CogniCodeProbe } from '../../../src/cognicode-probe-client';

export interface CogniCodePageProps {
  /** Base URL of the CogniCode Explorer (inner-loop product). */
  explorerBaseUrl?: string;
}

type State =
  | { kind: 'loading' }
  | { kind: 'ready'; probe: CogniCodeProbe }
  | { kind: 'unavailable'; message: string };

export function CogniCodePage(props: CogniCodePageProps = {}) {
  const [state, setState] = useState<State>({ kind: 'loading' });

  useEffect(() => {
    let cancelled = false;
    fetch('/api/cognicode/probe', { headers: { accept: 'application/json' } })
      .then(async response => {
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`);
        }
        return (await response.json()) as CogniCodeProbe;
      })
      .then(probe => {
        if (!cancelled) setState({ kind: 'ready', probe });
      })
      .catch(error => {
        if (!cancelled) {
          setState({ kind: 'unavailable', message: String(error) });
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  if (state.kind === 'loading') {
    return <div>CogniCode: connecting…</div>;
  }
  if (state.kind === 'unavailable') {
    // Unavailable is shown as unavailable, never as stale truth.
    return <div>CogniCode unavailable: {state.message}</div>;
  }

  const { probe } = state;
  const capsule = buildContextCapsule({
    workspace_id: probe.workspace_id ?? '',
    snapshot_id: probe.analysis_identity ?? undefined,
  });
  const href = props.explorerBaseUrl ? explorerDeepLink(props.explorerBaseUrl, capsule) : undefined;

  return (
    <div>
      <h1>CogniCode</h1>
      <p>Connected</p>
      <dl>
        <dt>Workspace</dt>
        <dd>{probe.workspace_id ?? '(none open)'}</dd>
        <dt>Service</dt>
        <dd>
          {probe.service} {probe.service_version}
        </dd>
        <dt>Capabilities</dt>
        <dd>{probe.capabilities.join(', ')}</dd>
      </dl>
      {href ? <a href={href}>Open Explorer</a> : <span>Open Explorer unavailable (no Explorer base URL)</span>}
    </div>
  );
}
