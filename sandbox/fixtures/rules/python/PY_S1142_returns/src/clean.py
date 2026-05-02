# Clean: Single return point
def check_value(x):
    result = "other"
    if x == 0:
        result = "zero"
    elif x == 1:
        result = "one"
    return result
