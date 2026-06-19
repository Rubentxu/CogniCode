// Groovy test fixture
def compute(x) {
    x * 2
}

def greet(name) {
    println "Hello, $name"
}

def main() {
    def result = compute(42)
    greet("world")
    println result
}

main()
