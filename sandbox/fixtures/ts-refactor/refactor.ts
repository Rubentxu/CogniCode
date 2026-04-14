/**
 * Refactoring Test Fixture - TypeScript
 *
 * Contains functions and structures for testing safe_refactor actions.
 * Ground truth: tests must pass after each refactoring.
 */

/**
 * Calculates the sum of two numbers.
 */
export function add(a: number, b: number): number {
  return a + b;
}

/**
 * Calculates the difference of two numbers.
 */
export function subtract(a: number, b: number): number {
  return a - b;
}

/**
 * Calculates the product of two numbers.
 */
export function multiply(a: number, b: number): number {
  return a * b;
}

/**
 * Calculates the quotient of two numbers.
 */
export function divide(a: number, b: number): number {
  return Math.floor(a / b);
}

/**
 * A simple calculator class.
 */
export class SimpleCalc {
  private value: number;

  constructor() {
    this.value = 0;
  }

  setValue(val: number): void {
    this.value = val;
  }

  getValue(): number {
    return this.value;
  }

  addAmount(amount: number): void {
    this.value += amount;
  }
}
