# Clean: Except block with logging
import logging

logger = logging.getLogger(__name__)

def handle():
    try:
        risky_operation()
    except Exception as e:
        logger.warning(f"Operation failed: {e}")
