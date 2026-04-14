"""
Indexing Test Fixture - Python (main module)

Ground truth symbols:
  - greet (function) line 9
  - farewell (function) line 13
  - Calculator (class) line 17
  - Calculator.__init__ (method) line 18
  - Calculator.add (method) line 22
  - Calculator.result (method) line 25
  - process (function) line 29
"""

from utils import format_result, validate_input


def greet(name: str) -> str:
    return f"Hello, {name}!"


def farewell(name: str) -> str:
    return f"Goodbye, {name}!"


class Calculator:
    def __init__(self):
        self._value = 0

    def add(self, n: int) -> None:
        self._value += n

    def result(self) -> int:
        return self._value


def process(items: list) -> list:
    return [item.upper() for item in items]
