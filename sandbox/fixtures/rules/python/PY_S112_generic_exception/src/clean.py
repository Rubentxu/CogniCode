# Clean: Specific exception
def validate(data):
    if not data:
        raise ValueError("Data is required")
