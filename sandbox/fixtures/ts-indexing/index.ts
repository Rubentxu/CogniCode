/**
 * Indexing Test Fixture - TypeScript (main module)
 *
 * Ground truth symbols:
 *   - greet (function) line 9
 *   - farewell (function) line 13
 *   - Calculator (class) line 18
 *   - Calculator.constructor (method) line 19
 *   - Calculator.add (method) line 23
 *   - Calculator.result (method) line 27
 *   - process (function) line 32
 */

export function greet(name: string): string {
  return `Hello, ${name}!`;
}

export function farewell(name: string): string {
  return `Goodbye, ${name}!`;
}

export class Calculator {
  private value: number;

  constructor() {
    this.value = 0;
  }

  add(n: number): void {
    this.value += n;
  }

  result(): number {
    return this.value;
  }
}

export function process(items: string[]): string[] {
  return items.map(item => item.toUpperCase());
}

export { formatResult, validateInput } from './utils';
