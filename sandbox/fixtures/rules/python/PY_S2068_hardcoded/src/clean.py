# Clean: Using environment variable
import os
def get_password():
    return os.environ.get("PASSWORD", "")
