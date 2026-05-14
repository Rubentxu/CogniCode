// Smelly: Exception thrown without chaining
public class Handler {
    public void handle() {
        try {
            doSomething();
        } catch (Exception e) {
            throw new RuntimeException("Operation failed");  // No chaining!
        }
    }

    private void doSomething() throws Exception {}
}
