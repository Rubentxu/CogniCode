/**
 * A simple greeting module.
 */

function greet_user(name) {
  return `Hello, ${name}!`;
}

function add(a, b) {
  return a + b;
}

function subtract(a, b) {
  return a - b;
}

module.exports = { greet, add, subtract };

if (require.main === module) {
  console.log(greet('CogniCode'));
  console.log(`2 + 3 = ${add(2, 3)}`);
}
