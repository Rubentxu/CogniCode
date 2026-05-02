# Smelly: Broad except
def process_data(data):
    try:
        return int(data)
    except Exception:
        return 0
