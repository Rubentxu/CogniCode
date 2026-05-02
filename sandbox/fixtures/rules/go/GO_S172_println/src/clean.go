// Clean: Using logger instead of fmt.Println
package main

import "log"

func processData(data []int) []int {
	log.Printf("Processing data: %v", data)
	result := make([]int, len(data))
	for i, v := range data {
		result[i] = v * 2
	}
	return result
}
