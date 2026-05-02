# Clean: Static regex
import re
PATTERN = re.compile(r"^\w+$")
def validate(value):
    return PATTERN.match(value)
