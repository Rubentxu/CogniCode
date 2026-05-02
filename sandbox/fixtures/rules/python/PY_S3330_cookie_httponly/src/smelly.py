# Smelly: Cookie without HttpOnly
from flask import make_response
def set_cookie():
    resp = make_response("OK")
    resp.set_cookie("session", "abc123")
    return resp
