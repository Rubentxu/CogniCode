/**
 * Indexing Test Fixture - TypeScript (utils module)
 *
 * Ground truth symbols:
 *   - formatResult (function) line 7
 *   - validateInput (function) line 11
 *   - MAX_RETRIES (constant) line 16
 */

export function formatResult(value: number): string {
  return `Result: ${value}`;
}

export function validateInput(input: unknown): boolean {
  return typeof input === 'string' && input.length > 0;
}

export const MAX_RETRIES = 3;
