# Clean: No cookie setting in this function
from flask import make_response
def set_response():
    resp = make_response("OK")
    return resp
