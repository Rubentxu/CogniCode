# Smelly: Catching overly broad Exception
def handle():
    try:
        do_something()
    except Exception:
        # Too broad!
        pass
