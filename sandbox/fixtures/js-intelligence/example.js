/**
 * Code Intelligence Test Fixture - JavaScript
 * 
 * This file is designed for testing symbol extraction, outline generation,
 * and symbol code retrieval. Ground truth is documented in the manifest.
 */

/**
 * A simple greeting function.
 * @param {string} name - The name to greet
 * @returns {string} The greeting message
 */
function greet(name) {
    return `Hello, ${name}!`;
}

/**
 * Adds two numbers.
 * @param {number} a - First number
 * @param {number} b - Second number
 * @returns {number} The sum
 */
function add(a, b) {
    return a + b;
}

/**
 * Multiplies two numbers.
 * @param {number} a - First number
 * @param {number} b - Second number
 * @returns {number} The product
 */
function multiply(a, b) {
    return a * b;
}

/**
 * Internal helper function (private).
 * @param {number} x - The input value
 * @returns {number} The doubled value
 */
function _helperInternal(x) {
    return x * 2;
}

/**
 * A calculator class demonstrating class methods.
 */
class Calculator {
    /**
     * Creates a new calculator with initial value.
     * @param {number} initial - The initial value (default: 0)
     */
    constructor(initial = 0) {
        this.value = initial;
    }

    /**
     * Adds to the current value.
     * @param {number} amount - The amount to add
     */
    add(amount) {
        this.value += amount;
    }

    /**
     * Gets the current value.
     * @returns {number} The current value
     */
    getValue() {
        return this.value;
    }
}

/**
 * Main entry point.
 */
function main() {
    const calc = new Calculator();
    calc.add(10);
    console.log(`Result: ${calc.getValue()}`);
}

// Export for module usage
module.exports = {
    greet,
    add,
    multiply,
    Calculator,
    main
};

// Run if executed directly
if (require.main === module) {
    main();
}
