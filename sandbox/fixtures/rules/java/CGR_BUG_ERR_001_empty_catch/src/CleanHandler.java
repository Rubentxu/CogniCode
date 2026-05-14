// Clean: Catch block with logging
import java.util.logging.Logger;

public class ExceptionHandler {
    private static final Logger logger = Logger.getLogger(ExceptionHandler.class.getName());

    public void handle() {
        try {
            riskyOperation();
        } catch (Exception e) {
            logger.warning("Operation failed: " + e.getMessage());
        }
    }

    private void riskyOperation() throws Exception {}
}
