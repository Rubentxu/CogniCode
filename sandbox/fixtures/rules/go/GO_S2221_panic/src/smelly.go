// Smelly: panic in non-test code
package main

func process(value int) {
	if value < 0 {
		panic("negative value not allowed")
	}
}
