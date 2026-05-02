// Clean: Using a struct for parameters
package main

type UserParams struct {
	A, B, C, D, E int
}

func createUser(params UserParams) map[string]int {
	return map[string]int{"a": params.A, "b": params.B}
}
