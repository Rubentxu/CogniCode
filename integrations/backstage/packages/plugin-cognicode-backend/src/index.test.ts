import { describe, expect, it, vi } from 'vitest';
import { createCognicodeRouter } from './index';

const probeBody = {
  service: 'cognicode-explorer',
  service_version: '0.5.0',
  workspace_id: 'ws-1',
  analysis_identity: null,
  symbol_count: 1,
  relation_count: 2,
  capabilities: ['explorer'],
};

/** Minimal express-less harness: invoke the registered GET /probe handler. */
async function callProbe(router: any, headers: Record<string, string> = {}) {
  const layer = router.stack.find((l: any) => l.route?.path === '/probe');
  const handler = layer.route.stack[0].handle;
  let statusCode = 200;
  let payload: unknown;
  const res = {
    status(code: number) { statusCode = code; return this; },
    json(body: unknown) { payload = body; return this; },
  };
  await handler({ header: (n: string) => headers[n] }, res);
  return { statusCode, payload };
}

describe('CogniCode backend plugin (CP0)', () => {
  it('proxies a real service response', async () => {
    const fetchImpl = vi.fn(async () => ({ ok: true, status: 200, json: async () => probeBody })) as any;
    const router = createCognicodeRouter({
      baseUrl: 'http://cn:7000',
      logger: { warn: vi.fn(), error: vi.fn() },
      fetchImpl,
    });
    const { statusCode, payload } = await callProbe(router);
    expect(statusCode).toBe(200);
    expect(payload).toMatchObject({ service: 'cognicode-explorer', service_version: '0.5.0' });
  });

  it('propagates a Backstage actor as identity context only', async () => {
    const fetchImpl = vi.fn(async () => ({ ok: true, status: 200, json: async () => probeBody })) as any;
    const router = createCognicodeRouter({
      baseUrl: 'http://cn:7000',
      logger: { warn: vi.fn(), error: vi.fn() },
      fetchImpl,
    });
    await callProbe(router, { 'x-cognicode-actor-subject': 'alice' });
    const [, init] = fetchImpl.mock.calls[0];
    expect(init.headers['x-cognicode-actor-subject']).toBe('alice');
    expect(init.headers['x-cognicode-actor-source']).toBe('backstage');
  });

  it('reports 503 unavailable rather than serving stale data', async () => {
    const fetchImpl = vi.fn(async () => {
      throw new Error('ECONNREFUSED');
    }) as any;
    const router = createCognicodeRouter({
      baseUrl: 'http://cn:7000',
      logger: { warn: vi.fn(), error: vi.fn() },
      fetchImpl,
    });
    const { statusCode, payload } = await callProbe(router);
    expect(statusCode).toBe(503);
    expect(payload).toEqual({ status: 'unavailable', service: 'cognicode' });
  });

  it('keeps no state between requests (restart/replica safe)', async () => {
    let version = '1.0.0';
    const fetchImpl = vi.fn(async () => ({
      ok: true,
      status: 200,
      json: async () => ({ ...probeBody, service_version: version }),
    })) as any;
    const router = createCognicodeRouter({
      baseUrl: 'http://cn:7000',
      logger: { warn: vi.fn(), error: vi.fn() },
      fetchImpl,
    });
    expect((await callProbe(router)).payload).toMatchObject({ service_version: '1.0.0' });
    version = '2.0.0';
    expect((await callProbe(router)).payload).toMatchObject({ service_version: '2.0.0' });
  });
});
