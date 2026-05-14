# Clean: Exception with proper chaining
def handle():
    try:
        do_something()
    except Exception as e:
        raise RuntimeError("Operation failed") from e  # Chained!
