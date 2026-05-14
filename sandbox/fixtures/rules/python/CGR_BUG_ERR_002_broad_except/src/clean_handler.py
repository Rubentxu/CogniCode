# Clean: Catching specific exception
def handle():
    try:
        do_something()
    except FileNotFoundError as e:
        print(f"File not found: {e}")
