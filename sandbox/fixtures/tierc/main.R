# R test fixture
compute <- function(x) { x * 2 }
greet <- function(name) { cat("Hello, ", name, "\n") }
main <- function() { result <- compute(42); greet("world"); cat(result, "\n") }
main()
