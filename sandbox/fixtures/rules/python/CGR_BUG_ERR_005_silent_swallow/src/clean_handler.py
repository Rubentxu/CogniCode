# Clean: Proper exception handling
import logging

logger = logging.getLogger(__name__)

def handle():
    try:
        do_something()
    except Exception as e:
        logger.warning(f"Operation failed: {e}")
        handle_error(e)

def handle_error(e):
    # Recovery logic
    pass
