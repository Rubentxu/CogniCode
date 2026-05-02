# Smelly: High cognitive complexity
def process_data(data, condition1, condition2, condition3):
    if condition1:
        if condition2:
            for item in data:
                if item > 0:
                    while True:
                        try:
                            if condition3:
                                return item
                            break
                        except:
                            continue
    return None
