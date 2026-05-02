// Hardcoded credentials in library code
package main

import "log"

func process(value int) {
	if value < 0 {
		log.Fatal("negative value not allowed")
	}
}
