// Smelly: Return in finally block
function handle() {
    try {
        return doSomething();
    } finally {
        return "default";  // This suppresses exceptions!
    }
}
