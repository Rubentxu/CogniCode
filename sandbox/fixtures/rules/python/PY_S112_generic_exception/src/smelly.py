# Smelly: Generic exception
def validate(data):
    if not data:
        raise Exception("Data is required")
