# Clean: Safe file permissions
import os
def set_permissions(path):
    os.chmod(path, 0o644)
