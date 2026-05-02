# Smelly: Exception swallowed
def process_data(data):
    try:
        return int(data)
    except:
        pass
