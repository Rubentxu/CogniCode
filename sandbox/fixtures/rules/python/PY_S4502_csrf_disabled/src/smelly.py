# Smelly: CSRF protection disabled
from flask_wtf import csrf
@app.route("/upload", methods=["POST"])
@csrf.exempt
def upload():
    return "OK"
