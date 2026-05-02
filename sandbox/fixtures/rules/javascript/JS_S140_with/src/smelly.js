// Smelly: Using with statement - ambiguous scope
function processObj(obj) {
    with (obj) {
        console.log(x);
        console.log(y);
    }
}
