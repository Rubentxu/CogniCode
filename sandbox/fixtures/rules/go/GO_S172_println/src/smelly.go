// Smelly: fmt.Println in library code
package main

import "fmt"

func processData(data []int) []int {
	fmt.Println("Processing data:", data)
	result := make([]int, len(data))
	for i, v := range data {
		result[i] = v * 2
	}
	return result
}
