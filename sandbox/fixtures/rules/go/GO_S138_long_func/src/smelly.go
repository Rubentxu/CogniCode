// Smelly: Long function
package main

func processItems(items []int) []int {
	result := make([]int, 0)
	for _, item := range items {
		if item > 0 {
			result = append(result, item*2)
		} else if item == 0 {
			result = append(result, 0)
		} else {
			result = append(result, item+10)
		}
	}
	return result
}
