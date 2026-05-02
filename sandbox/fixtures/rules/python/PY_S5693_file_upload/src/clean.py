# Clean: File upload with size limit
from flask import request
MAX_SIZE = 1024 * 1024
def upload_file():
    file = request.files["file"] if request.content_length <= MAX_SIZE else None
    return file.read()
