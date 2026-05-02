# Smelly: Catching BaseException
def handle_error():
    try:
        process()
    except BaseException:
        print("Error occurred")
