# Clean: CSP header present
from flask import Flask, make_response
app = Flask(__name__)
@app.route("/")
def index():
    resp = make_response("Hello")
    resp.headers["Content-Security-Policy"] = "default-src 'self'"
    return resp
