/**
 * Refactoring Test Fixture - JavaScript
 *
 * Contains functions and structures for testing safe_refactor actions.
 * Ground truth: tests must pass after each refactoring.
 */

/**
 * Calculates the sum of two numbers.
 */
function add(a, b) {
  return a + b;
}

/**
 * Calculates the difference of two numbers.
 */
function subtract(a, b) {
  return a - b;
}

/**
 * Calculates the product of two numbers.
 */
function multiply(a, b) {
  return a * b;
}

/**
 * Calculates the quotient of two numbers.
 */
function divide(a, b) {
  return Math.floor(a / b);
}

/**
 * A simple calculator class.
 */
class SimpleCalc {
  constructor() {
    this.value = 0;
  }

  setValue(val) {
    this.value = val;
  }

  getValue() {
    return this.value;
  }

  addAmount(amount) {
    this.value += amount;
  }
}

module.exports = { add, subtract, multiply, divide, SimpleCalc };
