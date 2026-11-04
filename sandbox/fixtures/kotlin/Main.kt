package com.example.shapes

fun main() {
    val shapes = listOf(Rect(1.0, 2.0), Circle.unit())
    shapes.forEach { println(describeShape(it)) }
}

fun describeShape(shape: Shape): String = shape.describe()
