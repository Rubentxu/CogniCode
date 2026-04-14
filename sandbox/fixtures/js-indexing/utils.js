/**
 * Indexing Test Fixture - JavaScript (utils module)
 *
 * Ground truth symbols:
 *   - formatResult (function) line 7
 *   - validateInput (function) line 11
 *   - MAX_RETRIES (constant) line 16
 */

function formatResult(value) {
  return `Result: ${value}`;
}

function validateInput(input) {
  return typeof input === 'string' && input.length > 0;
}

const MAX_RETRIES = 3;

module.exports = { formatResult, validateInput, MAX_RETRIES };
