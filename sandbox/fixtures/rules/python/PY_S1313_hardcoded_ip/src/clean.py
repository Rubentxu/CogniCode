# Clean: Config from environment
import os
def get_config():
    return {"server": os.environ.get("SERVER", "localhost"), "port": 8080}
