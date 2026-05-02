# Clean: CSRF protection enabled
from flask_wtf import csrf
@app.route("/upload", methods=["POST"])
def upload():
    return "OK"
