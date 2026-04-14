"""Tests for refactoring fixture - behavioral preservation."""

from refactor import add, subtract, multiply, divide, SimpleCalc


def test_add():
    assert add(2, 3) == 5
    assert add(-1, 1) == 0
    assert add(0, 0) == 0


def test_subtract():
    assert subtract(5, 3) == 2
    assert subtract(3, 5) == -2


def test_multiply():
    assert multiply(3, 4) == 12
    assert multiply(-2, 3) == -6


def test_divide():
    assert divide(10, 2) == 5
    assert divide(9, 3) == 3


def test_simple_calc():
    calc = SimpleCalc()
    assert calc.get_value() == 0
    calc.set_value(10)
    assert calc.get_value() == 10
    calc.add_amount(5)
    assert calc.get_value() == 15


if __name__ == "__main__":
    test_add()
    test_subtract()
    test_multiply()
    test_divide()
    test_simple_calc()
    print("All tests passed!")
