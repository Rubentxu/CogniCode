# Julia test fixture
function compute(x)
    return x * 2
end

function greet(name)
    println("Hello, ", name)
end

function main()
    result = compute(42)
    greet("world")
    println(result)
end

main()
