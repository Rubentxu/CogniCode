/** Tests for the hello TypeScript module. */

import { greet, add, subtract } from './hello';

test('greet returns greeting string', () => {
  expect(greet('World')).toBe('Hello, World!');
});

test('add adds two numbers', () => {
  expect(add(2, 3)).toBe(5);
});

test('subtract subtracts two numbers', () => {
  expect(subtract(5, 3)).toBe(2);
});
