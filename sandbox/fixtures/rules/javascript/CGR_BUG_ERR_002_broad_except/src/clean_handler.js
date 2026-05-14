// Clean: Catching specific error type
function handle() {
    try {
        doSomething();
    } catch (e) {
        if (e instanceof TypeError) {
            console.error("Type error:", e.message);
        }
    }
}
