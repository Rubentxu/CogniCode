/**
 * TS Analysis Fixture — Multi-module TypeScript project for analysis scenarios
 * Used for analysis-class scenarios: extract_symbols, find_references, call_graph
 */

export class DataProcessor<T> {
  constructor(private name: string) {
    this.data = [];
  }

  private data: T[];

  add(item: T): this {
    this.data.push(item);
    return this;
  }

  getCount(): number {
    return this.data.length;
  }

  process<R>(fn: (item: T) => R): R[] {
    return this.data.map(fn);
  }

  filter(predicate: (item: T) => boolean): T[] {
    return this.data.filter(predicate);
  }
}

export class StringUtils {
  static capitalize(str: string): string {
    return str.charAt(0).toUpperCase() + str.slice(1);
  }

  static truncate(str: string, maxLen: number): string {
    return str.length > maxLen ? str.slice(0, maxLen) + '...' : str;
  }
}

export function createProcessor<T>(name: string): DataProcessor<T> {
  return new DataProcessor<T>(name);
}
