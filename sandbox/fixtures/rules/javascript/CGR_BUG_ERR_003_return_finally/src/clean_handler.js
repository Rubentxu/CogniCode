// Clean: No return in finally
function handle() {
    let result;
    try {
        result = doSomething();
    } finally {
        cleanup();  // Just cleanup, no return
    }
    return result;
}
