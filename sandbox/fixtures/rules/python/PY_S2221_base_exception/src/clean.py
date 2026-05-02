# Clean: Catching specific exception
def handle_error():
    try:
        process()
    except RuntimeError as e:
        print(f"Error: {e}")
