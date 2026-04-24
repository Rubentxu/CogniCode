"""Python Debug Fixture

This module contains various crash scenarios for testing debug_analyze().
Run with: python crash_demo.py <crash_type>
"""

import sys
import os


def crash_index_error():
    """IndexError - list index out of range"""
    v = [1, 2, 3]
    return v[10]


def crash_key_error():
    """KeyError - dictionary key not found"""
    d = {"a": 1, "b": 2}
    return d["z"]


def crash_type_error():
    """TypeError - unsupported operand types"""
    return "str" + 123


def crash_value_error():
    """ValueError - invalid value"""
    int("not_a_number")


def crash_attribute_error():
    """AttributeError - module has no attribute"""
    return os.nonexistent


def crash_file_not_found():
    """FileNotFoundError"""
    with open("/nonexistent/file.txt") as f:
        return f.read()


def crash_custom_exception():
    """Custom exception"""
    raise RuntimeError("This is a custom error for testing")


def crash_recursion():
    """RecursionError - maximum recursion depth exceeded"""
    def recurse():
        return recurse()
    recurse()


def crash_import_error():
    """ImportError - no module named"""
    import nonexistent_module


def crash_zero_division():
    """ZeroDivisionError"""
    return 1 / 0


CRASH_FUNCTIONS = {
    "index_error": crash_index_error,
    "key_error": crash_key_error,
    "type_error": crash_type_error,
    "value_error": crash_value_error,
    "attribute_error": crash_attribute_error,
    "file_not_found": crash_file_not_found,
    "custom_exception": crash_custom_exception,
    "recursion": crash_recursion,
    "import_error": crash_import_error,
    "zero_division": crash_zero_division,
}


def main():
    if len(sys.argv) < 2:
        print("Usage: python crash_demo.py <crash_type>")
        print("Available crash types:")
        for name in CRASH_FUNCTIONS:
            print(f"  {name}")
        sys.exit(1)

    crash_type = sys.argv[1]
    if crash_type not in CRASH_FUNCTIONS:
        print(f"Unknown crash type: {crash_type}")
        sys.exit(1)

    func = CRASH_FUNCTIONS[crash_type]
    try:
        func()
    except Exception as e:
        # Print to stderr for capture
        print(f"Exception: {type(e).__name__}: {e}", file=sys.stderr)
        raise


if __name__ == "__main__":
    main()
