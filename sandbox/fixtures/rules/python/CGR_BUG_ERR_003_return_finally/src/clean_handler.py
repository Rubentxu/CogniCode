# Clean: No return in finally
def handle():
    try:
        result = do_something()
    finally:
        cleanup()  # Just cleanup, no return
    return result
