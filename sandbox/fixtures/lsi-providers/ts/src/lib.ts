export function helper(value: number): number {
  return value + 1;
}

export function main(): number {
  const doubled = helper(2);
  return doubled * 2;
}
