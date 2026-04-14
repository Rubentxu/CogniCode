// Package main provides a simple example Go module for CogniCode sandbox testing.
package main

import "fmt"

// Greet returns a greeting string for the given name.
func Greet(name string) string {
	return fmt.Sprintf("Hello, %s!", name)
}

// Add adds two integers and returns the result.
func Add(a, b int) int {
	return a + b
}

// Subtract subtracts b from a.
func Subtract(a, b int) int {
	return a - b
}

// Multiply multiplies two integers (used for mutation testing only).
func Multiply(a, b int) int {
	return a * b
}

func main() {
	fmt.Println(Greet("CogniCode"))
	fmt.Printf("2 + 3 = %d\n", Add(2, 3))
}
