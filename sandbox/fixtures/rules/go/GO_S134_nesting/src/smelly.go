// Smelly: Deeply nested ifs
package main

func checkValues(a, b, c, d, e, f bool) bool {
	if a {
		if b {
			if c {
				if d {
					if e {
						if f {
							return true
						}
					}
				}
			}
		}
	}
	return false
}
