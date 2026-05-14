# Smelly: Exception raised without chaining
def handle():
    try:
        do_something()
    except Exception as e:
        raise RuntimeError("Operation failed")  # No chaining!
