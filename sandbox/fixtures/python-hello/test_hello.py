"""Tests for the hello module."""

import pytest
from hello import greet, add, subtract


def test_greet():
    assert greet("World") == "Hello, World!"


def test_add():
    assert add(2, 3) == 5


def test_subtract():
    assert subtract(5, 3) == 2
