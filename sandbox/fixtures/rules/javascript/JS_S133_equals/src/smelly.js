// Smelly: Using == instead of === - type coercion issues
function checkEquality(x, y) {
    if (x == y) {
        return true;
    }
    return false;
}
