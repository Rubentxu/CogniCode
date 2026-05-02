// Clean: Dictionary-based approach
package main

var mapping = map[int]string{
	1: "one",
	2: "two",
	3: "three",
}

func classify(value int) string {
	if result, ok := mapping[value]; ok {
		return result
	}
	return "other"
}
