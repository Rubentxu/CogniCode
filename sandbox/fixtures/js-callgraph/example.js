/**
 * Call Graph Test Fixture - JavaScript
 * 
 * This fixture has known call relationships for testing call graph tools.
 * 
 * Call graph structure:
 *   main → helper → compute
 *   main → process
 * 
 * Ground truth:
 *   - Entry points: [main]
 *   - Leaf functions: [compute, process, _helperInternal]
 *   - Edges: main→helper, helper→compute, main→process, helper→_helperInternal
 */

/**
 * Main entry point - calls helper and process.
 */
function main() {
    const result = helper(42);
    process(result);
}

/**
 * Helper function - calls compute and _helperInternal.
 * @param {number} x - Input value
 * @returns {number} Result after processing
 */
function helper(x) {
    const a = compute(x);
    return _helperInternal(a);
}

/**
 * Compute function - leaf node (no outgoing edges).
 * @param {number} x - Input value
 * @returns {number} Doubled value
 */
function compute(x) {
    return x * 2;
}

/**
 * Process function - leaf node (no outgoing edges).
 * @param {number} value - Value to process
 */
function process(value) {
    console.log(`Result: ${value}`);
}

/**
 * Internal helper - leaf node (private, no outgoing edges).
 * @param {number} x - Input value
 * @returns {number} Incremented value
 */
function _helperInternal(x) {
    return x + 1;
}

// Export for module usage
module.exports = {
    main,
    helper,
    compute,
    process,
    _helperInternal
};

// Run if executed directly
if (require.main === module) {
    main();
}
