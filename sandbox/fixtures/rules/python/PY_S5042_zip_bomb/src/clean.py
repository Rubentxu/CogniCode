# Clean: Zip extraction with validation
import tarfile
def extract_archive(path):
    with tarfile.open(path) as tar:
        for member in tar.getmembers():
            if member.size > 1000000:
                raise ValueError("File too large")
        tar.extractall("/tmp/extracted", members=tar.getmembers())
