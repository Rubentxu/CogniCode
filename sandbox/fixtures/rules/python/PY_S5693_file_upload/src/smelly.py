# Smelly: File upload without size limit
from flask import request
def upload_file():
    file = request.files["file"]
    return file.read()
