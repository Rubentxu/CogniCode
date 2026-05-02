# Smelly: Long function
def process_items(items):
    result = []
    for item in items:
        if item > 0:
            result.append(item * 2)
        elif item == 0:
            result.append(0)
        else:
            result.append(item + 10)
    return result
