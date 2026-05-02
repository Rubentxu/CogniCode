// Smelly: High cognitive complexity
package main

func processData(data []int, cond1, cond2, cond3 bool) int {
	if cond1 {
		if cond2 {
			for _, item := range data {
				if item > 0 {
					for {
						if cond3 {
							return item
						}
						break
					}
				}
			}
		}
	}
	return 0
}
