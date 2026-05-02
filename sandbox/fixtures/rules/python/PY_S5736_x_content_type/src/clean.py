# Clean: X-Content-Type-Options present
from flask import Flask, make_response
app = Flask(__name__)
@app.route("/")
def index():
    resp = make_response("Hello")
    resp.headers["X-Content-Type-Options"] = "nosniff"
    return resp
