# Clean: Using environment variable
import os

def get_password():
    password = os.getenv("PASS")
    return password
