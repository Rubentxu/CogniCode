# Rust Debug Fixture

This fixture contains various crash scenarios for testing the `debug_analyze()` function.

## Building

```bash
cargo build --release
```

## Running Crash Scenarios

```bash
# Index out of bounds
cargo run --release -- index_oob

# Unwrap on None
cargo run --release -- unwrap_none

# Division by zero
cargo run --release -- divzero

# Assertion failure
cargo run --release -- assert

# Custom panic
cargo run --release -- custom_panic
```

## Crash Types

| Name | Description |
|------|-------------|
| `index_oob` | Vector index out of bounds |
| `unwrap_none` | Option::unwrap() on None |
| `expect_none` | Option::expect() on None |
| `divzero` | Division by zero |
| `assert` | Assertion failure |
| `custom_panic` | Custom panic message |
| `slice_oob` | Slice range out of bounds |
| `pop_empty` | Pop from empty vector |
| `overflow` | Integer overflow |
| `nested_unwrap` | Result::unwrap() on Err |
