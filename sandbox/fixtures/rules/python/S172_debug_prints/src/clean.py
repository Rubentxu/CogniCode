# Clean: Using logger instead of print
import logging

def process_data(data):
    logging.info("Processing data: %s", data)
    return [x * 2 for x in data]
