# Clean: Exception logged
import logging
logger = logging.getLogger(__name__)
def process_data(data):
    try:
        return int(data)
    except ValueError as e:
        logger.error("Failed to convert: %s", e)
        raise
