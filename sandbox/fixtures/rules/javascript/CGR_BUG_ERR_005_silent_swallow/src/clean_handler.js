// Clean: Proper exception handling
function handle() {
    try {
        doSomething();
    } catch (e) {
        console.warn("Operation failed:", e);
        handleError(e);
    }
}
