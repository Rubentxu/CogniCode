import { describe, expect, it } from 'vitest';
import {
  CONTEXT_CAPSULE_VERSION,
  ContextCapsuleError,
  MAX_FOCUS_REFS,
  buildContextCapsule,
  decodeContextCapsule,
  encodeContextCapsule,
  explorerDeepLink,
} from './context-capsule';

describe('ContextCapsule v1 (CP0 WU6)', () => {
  it('is versioned, bounded and references-only', () => {
    const c = buildContextCapsule({ workspace_id: 'cp0-fixture', focus: ['symbol:a', 'file:b.rs'] });
    expect(c.version).toBe(CONTEXT_CAPSULE_VERSION);
    expect(c.origin).toBe('backstage');
    expect(Object.keys(c).sort()).toEqual(['focus', 'origin', 'version', 'workspace_id']);
  });

  it('never carries authority-bearing fields', () => {
    const c = buildContextCapsule({ workspace_id: 'w' }) as Record<string, unknown>;
    for (const forbidden of [
      'approval',
      'authorization',
      'permit',
      'promotion',
      'actor',
      'requested_by',
      'facts',
      'evidence',
    ]) {
      expect(Object.keys(c)).not.toContain(forbidden);
    }
  });

  it('canonicalises focus deterministically (dedupe + sort)', () => {
    const a = buildContextCapsule({ workspace_id: 'w', focus: ['b', 'a', 'b'] });
    const b = buildContextCapsule({ workspace_id: 'w', focus: ['a', 'b'] });
    expect(a).toEqual(b);
    expect(a.focus).toEqual(['a', 'b']);
  });

  it('rejects invalid input rather than repairing it', () => {
    expect(() => buildContextCapsule({ workspace_id: '  ' })).toThrow(ContextCapsuleError);
    expect(() => buildContextCapsule({ workspace_id: 'w', focus: [''] })).toThrow(ContextCapsuleError);
    expect(() =>
      buildContextCapsule({ workspace_id: 'w', focus: ['x'.repeat(513)] }),
    ).toThrow(ContextCapsuleError);
    expect(() =>
      buildContextCapsule({
        workspace_id: 'w',
        focus: Array.from({ length: MAX_FOCUS_REFS + 1 }, (_, i) => `r${i}`),
      }),
    ).toThrow(ContextCapsuleError);
  });

  it('round-trips through base64url and rejects an unknown version', () => {
    const c = buildContextCapsule({ workspace_id: 'w', snapshot_id: 's1', focus: ['a'] });
    expect(decodeContextCapsule(encodeContextCapsule(c))).toEqual(c);

    const future = Buffer.from(JSON.stringify({ ...c, version: 99 }), 'utf8').toString('base64');
    expect(() => decodeContextCapsule(future)).toThrow(/unsupported capsule version/);
    expect(() => decodeContextCapsule('!!!not-base64-json!!!')).toThrow(ContextCapsuleError);
  });

  it('omits an empty snapshot id instead of emitting a blank', () => {
    const c = buildContextCapsule({ workspace_id: 'w', snapshot_id: '   ' });
    expect(c.snapshot_id).toBeUndefined();
  });

  it('deep-links to the Explorer with the capsule and no iframe', () => {
    const c = buildContextCapsule({ workspace_id: 'w', focus: ['a'] });
    const link = explorerDeepLink('http://localhost:5173/', c);
    expect(link.startsWith('http://localhost:5173/?capsule=')).toBe(true);
    expect(link).not.toContain('<iframe');
    expect(decodeContextCapsule(link.split('capsule=')[1])).toEqual(c);
  });
});
