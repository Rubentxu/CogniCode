/**
 * A simple greeting module (TypeScript).
 */

export function greet(name: string): string {
  return `Hello, ${name}!`;
}

export function add(a: number, b: number): number {
  return a + b;
}

export function subtract(a: number, b: number): number {
  return a - b;
}

if (require.main === module) {
  console.log(greet('CogniCode'));
  console.log(`2 + 3 = ${add(2, 3)}`);
}
