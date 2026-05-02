// Clean: Single return point
package main

func checkValue(x int) string {
	result := "other"
	if x == 0 {
		result = "zero"
	} else if x == 1 {
		result = "one"
	}
	return result
}
