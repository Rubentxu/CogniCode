// Smelly: Return in finally block
public class Handler {
    public String handle() {
        try {
            return doSomething();
        } finally {
            return "default";  // This suppresses exceptions!
        }
    }

    private String doSomething() {
        throw new RuntimeException("error");
    }
}
