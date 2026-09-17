import { describe, expect, it, vi } from 'vitest';
import {
  ACTOR_SOURCE_HEADER,
  ACTOR_SUBJECT_HEADER,
  CogniCodeUnavailableError,
  PROBE_PATH,
  fetchProbe,
} from './cognicode-probe-client';

const probeBody = {
  service: 'cognicode-explorer',
  service_version: '0.5.0',
  workspace_id: 'ws-1',
  analysis_identity: '2026-09-17T00:00:00Z',
  symbol_count: 12,
  relation_count: 30,
  capabilities: ['explorer', 'mcp'],
  actor_context: { actor_subject: 'alice', actor_source: 'backstage', authority: 'none' },
};

function ok(body: unknown = probeBody): Response {
  return { ok: true, status: 200, json: async () => body } as unknown as Response;
}

describe('CogniCode probe client (CP0 WU3/WU4/WU8/WU10/WU11)', () => {
  it('calls the real probe path over GET', async () => {
    const fetchImpl = vi.fn(async () => ok());
    await fetchProbe({ baseUrl: 'http://cn:7000/', fetchImpl: fetchImpl as unknown as typeof fetch });
    expect(fetchImpl).toHaveBeenCalledTimes(1);
    const [url, init] = fetchImpl.mock.calls[0] as [string, RequestInit];
    expect(url).toBe(`http://cn:7000${PROBE_PATH}`);
    expect(init.method).toBe('GET');
  });

  it('forwards an authenticated host actor as explicit identity CONTEXT', async () => {
    const fetchImpl = vi.fn(async () => ok());
    await fetchProbe({
      baseUrl: 'http://cn:7000',
      actor: { subject: 'alice', source: 'backstage' },
      fetchImpl: fetchImpl as unknown as typeof fetch,
    });
    const [, init] = fetchImpl.mock.calls[0] as [string, RequestInit];
    const headers = init.headers as Record<string, string>;
    expect(headers[ACTOR_SUBJECT_HEADER]).toBe('alice');
    expect(headers[ACTOR_SOURCE_HEADER]).toBe('backstage');
  });

  it('omits actor headers when the host has no authenticated actor', async () => {
    const fetchImpl = vi.fn(async () => ok());
    await fetchProbe({ baseUrl: 'http://cn:7000', fetchImpl: fetchImpl as unknown as typeof fetch });
    const [, init] = fetchImpl.mock.calls[0] as [string, RequestInit];
    expect(init.headers).not.toHaveProperty(ACTOR_SUBJECT_HEADER);
  });

  it('reports unavailability instead of stale data', async () => {
    const failing = vi.fn(async () => {
      throw new Error('ECONNREFUSED');
    });
    await expect(
      fetchProbe({ baseUrl: 'http://cn:7000', fetchImpl: failing as unknown as typeof fetch }),
    ).rejects.toBeInstanceOf(CogniCodeUnavailableError);

    const http500 = vi.fn(async () => ({ ok: false, status: 500 } as unknown as Response));
    await expect(
      fetchProbe({ baseUrl: 'http://cn:7000', fetchImpl: http500 as unknown as typeof fetch }),
    ).rejects.toThrow(/HTTP 500/);

    const malformed = vi.fn(async () => ({
      ok: true,
      status: 200,
      json: async () => {
        throw new Error('bad json');
      },
    } as unknown as Response));
    await expect(
      fetchProbe({ baseUrl: 'http://cn:7000', fetchImpl: malformed as unknown as typeof fetch }),
    ).rejects.toThrow(/malformed probe response/);

    const wrongContract = vi.fn(async () => ok({ nope: true }));
    await expect(
      fetchProbe({ baseUrl: 'http://cn:7000', fetchImpl: wrongContract as unknown as typeof fetch }),
    ).rejects.toThrow(/probe contract not satisfied/);
  });

  it('holds no canonical state: successive calls are independent', async () => {
    const first = vi.fn(async () => ok({ ...probeBody, service_version: '1.0.0' }));
    const second = vi.fn(async () => ok({ ...probeBody, service_version: '2.0.0' }));
    const a = await fetchProbe({ baseUrl: 'http://cn:7000', fetchImpl: first as unknown as typeof fetch });
    const b = await fetchProbe({ baseUrl: 'http://cn:7000', fetchImpl: second as unknown as typeof fetch });
    expect(a.service_version).toBe('1.0.0');
    expect(b.service_version).toBe('2.0.0');
  });

  it('reports that no authority crossed the boundary', async () => {
    const fetchImpl = vi.fn(async () => ok());
    const probe = await fetchProbe({
      baseUrl: 'http://cn:7000',
      actor: { subject: 'alice', source: 'backstage' },
      fetchImpl: fetchImpl as unknown as typeof fetch,
    });
    expect(probe.actor_context?.authority).toBe('none');
  });
});
