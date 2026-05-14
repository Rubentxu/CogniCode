// Clean: Exception with proper chaining
public class Handler {
    public void handle() {
        try {
            doSomething();
        } catch (Exception e) {
            throw new RuntimeException("Operation failed", e);  // Chained!
        }
    }

    private void doSomething() throws Exception {}
}
