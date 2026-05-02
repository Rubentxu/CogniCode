# Smelly: File without context manager
def read_file(path):
    f = open(path, "r")
    return f.read()
