"""Call Graph Test Fixture - Python

This fixture has known call relationships for testing call graph tools.

Call graph structure:
  main → helper → compute
  main → process

Ground truth:
  - Entry points: [main]
  - Leaf functions: [compute, process, _helper_internal]
  - Edges: main→helper, helper→compute, main→process, helper→_helper_internal
"""


def main():
    """Main entry point - calls helper and process."""
    result = helper(42)
    process(result)


def helper(x):
    """Helper function - calls compute and _helper_internal."""
    a = compute(x)
    return _helper_internal(a)


def compute(x):
    """Compute function - leaf node (no outgoing edges)."""
    return x * 2


def process(value):
    """Process function - leaf node (no outgoing edges)."""
    print(f"Result: {value}")


def _helper_internal(x):
    """Internal helper - leaf node (private, no outgoing edges)."""
    return x + 1


if __name__ == "__main__":
    main()
