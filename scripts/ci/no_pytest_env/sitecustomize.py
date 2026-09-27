"""Simulate the PR-CI runner environment: pytest is not installed.

The failure this reproduces is real. `scripts/product/test_oss_foundation.py`
and `test_profiles.py` delegated their `__main__` to `pytest.main()`, which
passed locally on the maintainer's machine and failed on ubuntu-latest with
`ModuleNotFoundError: No module named 'pytest'`.

Putting this on `PYTHONPATH` intercepts any attempt to import pytest, so
running the suite through it proves the harness no longer needs a test
framework. If any suite regresses to `pytest.main()`, this run fails with
the same error CI reported.
"""

import sys


class _BlockPytest:
    """Meta-path hook that makes `import pytest` fail.

    `find_spec` is the hook Python 3.4+ actually consults. The older
    `find_module`/`load_module` pair is dead code since 3.4 and silently
    ignored on modern interpreters, which would make this whole
    simulation a no-op that reports success while proving nothing.
    """

    def find_spec(self, fullname, path=None, target=None):
        if fullname == "pytest" or fullname.startswith("pytest."):
            raise ImportError("No module named 'pytest'")
        return None


sys.meta_path.insert(0, _BlockPytest())
