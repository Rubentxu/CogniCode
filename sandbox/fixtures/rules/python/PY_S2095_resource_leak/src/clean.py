# Clean: File with context manager
def read_file(path):
    with open(path, "r") as f:
        return f.read()
