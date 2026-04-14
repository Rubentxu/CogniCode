"""
Refactoring Test Fixture - Python

Contains functions and structures for testing safe_refactor actions.
Ground truth: tests must pass after each refactoring.
"""


def add(a: int, b: int) -> int:
    """Calculates the sum of two numbers."""
    return a + b


def subtract(a: int, b: int) -> int:
    """Calculates the difference of two numbers."""
    return a - b


def multiply(a: int, b: int) -> int:
    """Calculates the product of two numbers."""
    return a * b


def divide(a: int, b: int) -> int:
    """Calculates the quotient of two numbers."""
    return a // b


class SimpleCalc:
    """A simple calculator class."""

    def __init__(self):
        self._value = 0

    def set_value(self, val: int) -> None:
        """Sets the value."""
        self._value = val

    def get_value(self) -> int:
        """Gets the current value."""
        return self._value

    def add_amount(self, amount: int) -> None:
        """Adds to the value."""
        self._value += amount
