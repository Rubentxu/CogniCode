# Smelly: Silent exception swallowing
def handle():
    try:
        do_something()
    except Exception:
        pass  # Silently ignored
