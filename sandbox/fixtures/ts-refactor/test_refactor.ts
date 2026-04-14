import { add, subtract, multiply, divide, SimpleCalc } from './refactor';

// Tests for behavioral preservation
function testAdd() {
  if (add(2, 3) !== 5) throw new Error('add(2,3) should be 5');
  if (add(-1, 1) !== 0) throw new Error('add(-1,1) should be 0');
  if (add(0, 0) !== 0) throw new Error('add(0,0) should be 0');
}

function testSubtract() {
  if (subtract(5, 3) !== 2) throw new Error('subtract(5,3) should be 2');
  if (subtract(3, 5) !== -2) throw new Error('subtract(3,5) should be -2');
}

function testMultiply() {
  if (multiply(3, 4) !== 12) throw new Error('multiply(3,4) should be 12');
  if (multiply(-2, 3) !== -6) throw new Error('multiply(-2,3) should be -6');
}

function testDivide() {
  if (divide(10, 2) !== 5) throw new Error('divide(10,2) should be 5');
  if (divide(9, 3) !== 3) throw new Error('divide(9,3) should be 3');
}

function testSimpleCalc() {
  const calc = new SimpleCalc();
  if (calc.getValue() !== 0) throw new Error('initial value should be 0');
  calc.setValue(10);
  if (calc.getValue() !== 10) throw new Error('value should be 10');
  calc.addAmount(5);
  if (calc.getValue() !== 15) throw new Error('value should be 15');
}

// Run all tests
testAdd();
testSubtract();
testMultiply();
testDivide();
testSimpleCalc();
console.log('All tests passed!');
