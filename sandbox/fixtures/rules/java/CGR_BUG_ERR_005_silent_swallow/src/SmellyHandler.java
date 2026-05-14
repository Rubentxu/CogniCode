// Smelly: Empty catch block
public class Handler {
    public void handle() {
        try {
            doSomething();
        } catch (Exception e) {
            // Silently ignored
        }
    }

    private void doSomething() throws Exception {}
}
