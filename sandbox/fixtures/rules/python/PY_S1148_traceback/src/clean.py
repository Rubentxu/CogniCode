# Clean: Proper logging
import logging
logger = logging.getLogger(__name__)
def handle_error():
    logger.exception("An error occurred")
