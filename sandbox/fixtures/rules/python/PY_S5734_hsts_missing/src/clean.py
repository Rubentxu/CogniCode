# Clean: HSTS header present
from flask import Flask, make_response
app = Flask(__name__)
@app.route("/")
def index():
    resp = make_response("Hello")
    resp.headers["Strict-Transport-Security"] = "max-age=31536000"
    return resp
