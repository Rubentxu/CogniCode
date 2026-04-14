/**
 * TS Mutation Fixture — Minimal TypeScript with functions to mutate and test
 * Used for mutation-class scenarios: edit_file with concrete changes
 */

export function calculateArea(width: number, height: number): number {
  return width * height;
}

export function calculateVolume(length: number, width: number, height: number): number {
  return length * width * height;
}

export function greet(name: string): string {
  return `Hello, ${name}!`;
}

if (require.main === module) {
  console.log('Area:', calculateArea(5, 10));
  console.log('Volume:', calculateVolume(2, 3, 4));
  console.log(greet('CogniCode'));
}
