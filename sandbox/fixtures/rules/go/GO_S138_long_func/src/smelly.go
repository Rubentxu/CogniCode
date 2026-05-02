// Smelly: Long function
package main

func processAllData(items []int, conditions []bool, values []float64, names []string, counts []int) []int {
	result := make([]int, 0)
	for i, item := range items {
		if i < len(conditions) && conditions[i] {
			if item > 0 {
				result = append(result, item*2)
			} else if item == 0 {
				result = append(result, 0)
			} else {
				if i < len(values) {
					result = append(result, int(values[i])+10)
				} else {
					result = append(result, item+10)
				}
			}
		} else {
			if item < 0 {
				result = append(result, -item)
			} else {
				result = append(result, item)
			}
		}
	}
	if len(result) > 0 {
		for i := 0; i < len(result); i++ {
			if result[i] > 100 {
				result[i] = 100
			}
		}
	}
	if len(items) > 10 {
		for i := 0; i < len(items); i++ {
			if items[i] > 50 {
				items[i] = 50
			}
		}
	}
	if len(conditions) > 5 {
		for i := 0; i < len(conditions); i++ {
			if conditions[i] {
				conditions[i] = false
			}
		}
	}
	if len(values) > 8 {
		for i := 0; i < len(values); i++ {
			if values[i] > 10.0 {
				values[i] = 10.0
			}
		}
	}
	if len(names) > 3 {
		for i := 0; i < len(names); i++ {
			if len(names[i]) > 10 {
				names[i] = names[i][:10]
			}
		}
	}
	if len(counts) > 7 {
		for i := 0; i < len(counts); i++ {
			if counts[i] > 1000 {
				counts[i] = 1000
			}
		}
	}
	for i := 0; i < len(result); i++ {
		if result[i] < 0 {
			result[i] = 0
		}
	}
	return result
}

func main() {
	items := []int{1, 2, 3, 4, 5}
	conditions := []bool{true, false, true, false, true}
	values := []float64{1.1, 2.2, 3.3}
	names := []string{"a", "bb", "ccc"}
	count := []int{1, 2, 3}
	processAllData(items, conditions, values, names, count)
}
