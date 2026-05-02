// Smelly: Empty catch block
public class ExceptionHandler {
    public void handle() {
        try {
            riskyOperation();
        } catch (Exception e) {}
    }

    private void riskyOperation() throws Exception {}
}
