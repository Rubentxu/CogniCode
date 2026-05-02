// Smelly: Too many returns
package main

func checkValue(x int) string {
	if x == 0 {
		return "zero"
	}
	if x == 1 {
		return "one"
	}
	if x == 2 {
		return "two"
	}
	if x == 3 {
		return "three"
	}
	if x == 4 {
		return "four"
	}
	return "other"
}
