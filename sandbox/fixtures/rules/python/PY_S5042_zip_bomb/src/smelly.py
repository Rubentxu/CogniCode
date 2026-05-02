# Smelly: Zip extraction without validation
import tarfile
def extract_archive(path):
    with tarfile.open(path) as tar:
        tar.extractall("/tmp/extracted")
