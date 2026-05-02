# Clean: Dictionary-based approach
def classify(value):
    mapping = {1: "one", 2: "two", 3: "three"}
    return mapping.get(value, "other")
