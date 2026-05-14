// Clean: No return in finally
public class Handler {
    public String handle() {
        String result;
        try {
            result = doSomething();
        } finally {
            cleanup();  // Just cleanup, no return
        }
        return result;
    }

    private String doSomething() {
        return "success";
    }

    private void cleanup() {}
}
