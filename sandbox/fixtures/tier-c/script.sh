#!/usr/bin/env bash
# Bash test fixture

compute() {
    echo $(( $1 * 2 ))
}

greet() {
    echo "Hello, $1"
}

main() {
    local result
    result=$(compute 42)
    greet "world"
    echo "$result"
}

main
