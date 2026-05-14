// Smelly: Catching overly broad Exception
function handle() {
    try {
        doSomething();
    } catch (e) {
        // Too broad!
    }
}
