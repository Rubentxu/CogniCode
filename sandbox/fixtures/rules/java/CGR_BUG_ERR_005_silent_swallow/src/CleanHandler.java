// Clean: Proper exception handling
import java.util.logging.Logger;

public class Handler {
    private static final Logger logger = Logger.getLogger(Handler.class.getName());

    public void handle() {
        try {
            doSomething();
        } catch (Exception e) {
            logger.log(java.util.logging.Level.WARNING, "Operation failed", e);
            handleError(e);
        }
    }

    private void doSomething() throws Exception {}

    private void handleError(Exception e) {
        // Recovery logic
    }
}
