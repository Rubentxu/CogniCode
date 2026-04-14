/**
 * JS Mutation Fixture — Minimal JS with functions to mutate and test
 * Used for mutation-class scenarios: edit_file with concrete changes
 */

function calculateArea(width, height) {
  return width * height;
}

function calculateVolume(length, width, height) {
  return length * width * height;
}

function greet(name) {
  return `Hello, ${name}!`;
}

module.exports = { calculateArea, calculateVolume, greet };

if (require.main === module) {
  console.log('Area:', calculateArea(5, 10));
  console.log('Volume:', calculateVolume(2, 3, 4));
  console.log(greet('CogniCode'));
}
