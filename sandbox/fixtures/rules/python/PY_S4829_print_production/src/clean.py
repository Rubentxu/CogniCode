# Clean: Logging
import logging
logger = logging.getLogger(__name__)
def process_data(data):
    logger.info("Processing data: %s", data)
    return data
