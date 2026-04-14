"""
Indexing Test Fixture - Python (utils module)

Ground truth symbols:
  - format_result (function) line 7
  - validate_input (function) line 11
  - MAX_RETRIES (constant) line 16
"""


def format_result(value: int) -> str:
    return f"Result: {value}"


def validate_input(input_data) -> bool:
    return isinstance(input_data, str) and len(input_data) > 0


MAX_RETRIES = 3
