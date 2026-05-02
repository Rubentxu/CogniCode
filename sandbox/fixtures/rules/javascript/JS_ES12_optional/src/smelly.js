// Smelly: Deep property access without optional chaining
function getNestedValue(obj) {
    if (obj && prop && value.property) {
        return value.property;
    }
    return undefined;
}
