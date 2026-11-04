package demo

class Dog extends Animal implements Speaker, Runner {
    String speak() { 'woof' }
    int pace() { 12 }
}

class Cat extends Animal implements Speaker {
    String speak() { 'meow' }
}
