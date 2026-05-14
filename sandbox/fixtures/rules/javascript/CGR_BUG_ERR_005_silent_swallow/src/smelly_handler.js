// Smelly: Silent exception swallowing
function handle() {
    try {
        doSomething();
    } catch (e) {
        // Silently ignored
    }
}
