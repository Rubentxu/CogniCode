/**
 * Indexing Test Fixture - JavaScript (main module)
 *
 * Ground truth symbols:
 *   - greet (function) line 7
 *   - farewell (function) line 12
 *   - Calculator (class) line 17
 *   - Calculator.constructor (method) line 18
 *   - Calculator.add (method) line 22
 *   - Calculator.result (method) line 26
 *   - process (function) line 31
 */

function greet(name) {
  return `Hello, ${name}!`;
}

function farewell(name) {
  return `Goodbye, ${name}!`;
}

class Calculator {
  constructor() {
    this.value = 0;
  }

  add(n) {
    this.value += n;
  }

  result() {
    return this.value;
  }
}

function process(items) {
  return items.map(item => item.toUpperCase());
}

const { formatResult } = require('./utils');

module.exports = { greet, farewell, Calculator, process, formatResult };
