/**
 * JS Analysis Fixture — Multi-module JS project for analysis scenarios
 * Used for analysis-class scenarios: extract_symbols, find_references, call_graph
 */

class DataProcessor {
  constructor(name) {
    this.name = name;
    this.data = [];
  }

  add(item) {
    this.data.push(item);
    return this;
  }

  getCount() {
    return this.data.length;
  }

  process(fn) {
    return this.data.map(fn);
  }

  filter(predicate) {
    return this.data.filter(predicate);
  }
}

class StringUtils {
  static capitalize(str) {
    return str.charAt(0).toUpperCase() + str.slice(1);
  }

  static truncate(str, maxLen) {
    return str.length > maxLen ? str.slice(0, maxLen) + '...' : str;
  }
}

function createProcessor(name) {
  return new DataProcessor(name);
}

module.exports = { DataProcessor, StringUtils, createProcessor };
