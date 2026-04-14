"""Code Intelligence Test Fixture - Python

This file is designed for testing symbol extraction, outline generation,
and symbol code retrieval. Ground truth is documented in the manifest.
"""


def greet(name: str) -> str:
    """A simple greeting function."""
    return f"Hello, {name}!"


def add(a: int, b: int) -> int:
    """Adds two numbers."""
    return a + b


def multiply(a: int, b: int) -> int:
    """Multiplies two numbers."""
    return a * b


def _helper_internal(x: int) -> int:
    """Internal helper function (private)."""
    return x * 2


class Calculator:
    """A calculator class demonstrating class methods."""

    def __init__(self, initial: int = 0):
        """Creates a new calculator with initial value."""
        self.value = initial

    def add(self, amount: int) -> None:
        """Adds to the current value."""
        self.value += amount

    def get_value(self) -> int:
        """Gets the current value."""
        return self.value


def main():
    """Main entry point."""
    calc = Calculator()
    calc.add(10)
    print(f"Result: {calc.get_value()}")


if __name__ == "__main__":
    main()
