# Clean: Specific except
def process_data(data):
    try:
        return int(data)
    except ValueError:
        return 0
