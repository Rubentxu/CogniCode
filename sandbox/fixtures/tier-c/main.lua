-- Lua test fixture
function greet(name)
    return "Hello, " .. name
end

local function compute(x)
    return x * 2
end

function main()
    local result = compute(42)
    print(greet("world"))
    return result
end

main()
