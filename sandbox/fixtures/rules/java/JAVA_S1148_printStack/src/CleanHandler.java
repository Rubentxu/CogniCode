// Clean: Using proper logging
import java.util.logging.Logger;

public class ExceptionHandler {
    private static final Logger logger = Logger.getLogger(ExceptionHandler.class.getName());

    public void handle(Exception e) {
        logger.error("Exception occurred", e);
    }
}
