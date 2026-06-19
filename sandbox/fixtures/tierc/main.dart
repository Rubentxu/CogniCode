// Dart test fixture
int compute(int x) {
  return x * 2;
}

void greet(String name) {
  print('Hello, $name');
}

void main() {
  var result = compute(42);
  greet('world');
  print(result);
}
