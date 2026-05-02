// Clean: Test with assertions
test('should process data', () => {
    const result = processData([1, 2, 3]);
    expect(result).toEqual([2, 4, 6]);
});
