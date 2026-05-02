// Smelly: High cognitive complexity
package main

func processData(data []int, cond1, cond2, cond3, cond4, cond5 bool) int {
	if cond1 {
		if cond2 {
			for _, item := range data {
				if item > 0 {
					for i := 0; i < 10; i++ {
						if cond3 {
							if cond4 {
								if cond5 {
									return item + i
								}
							}
						}
					}
				}
			}
		} else if cond1 && cond2 {
			switch cond3 {
			case true:
				if cond4 || cond5 {
					return 1
				}
			case false:
				if !cond4 && !cond5 {
					return 2
				}
			}
		}
	} else {
		if cond2 || cond3 {
			for i, item := range data {
				if item > 10 && i > 5 {
					if cond4 && cond5 {
						return item
					}
				}
			}
		}
	}
	return 0
}

func main() {
	data := []int{1, 2, 3, 4, 5}
	processData(data, true, true, false, true, false)
}
