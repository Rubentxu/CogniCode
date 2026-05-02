// Clean: Using environment variable
package main

import "os"

func getPassword() string {
	password := os.Getenv("PASS")
	return password
}
