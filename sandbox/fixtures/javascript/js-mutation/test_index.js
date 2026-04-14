const { calculateArea, calculateVolume, greet } = require('./index');

describe('calculateArea', () => {
  test('computes rectangle area', () => {
    expect(calculateArea(5, 10)).toBe(50);
  });
});

describe('calculateVolume', () => {
  test('computes box volume', () => {
    expect(calculateVolume(2, 3, 4)).toBe(24);
  });
});

describe('greet', () => {
  test('returns greeting string', () => {
    expect(greet('World')).toBe('Hello, World!');
  });
});
