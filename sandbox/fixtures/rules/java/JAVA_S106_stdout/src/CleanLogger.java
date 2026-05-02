// Clean: Using proper logger
import java.util.logging.Logger;

public class Logger {
    private static final Logger logger = Logger.getLogger(Logger.class.getName());

    public void log(String message) {
        logger.info(message);
    }
}
