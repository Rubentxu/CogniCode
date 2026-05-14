// Clean: Catch block with logging
function handle() {
    try {
        riskyOperation();
    } catch (e) {
        console.warn("Operation failed:", e);
    }
}
