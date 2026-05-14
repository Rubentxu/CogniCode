# Smelly: Return in finally block
def handle():
    try:
        return do_something()
    finally:
        return "default"  # This suppresses exceptions!
