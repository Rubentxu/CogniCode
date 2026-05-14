// Smelly: Catching overly broad Exception
public class Handler {
    public void handle() {
        try {
            doSomething();
        } catch (Exception e) {
            // Too broad!
        }
    }

    private void doSomething() throws Exception {}
}
