package com.example.shapes

import kotlin.math.PI as Pi
import kotlin.math.max

/** A shape with an area. */
interface Shape {
    val area: Double
    fun describe(): String
}

data class Rect(val width: Double, val height: Double) : Shape {
    override val area: Double
        get() = width * height

    constructor(side: Double) : this(side, side)

    class Filled {
        val label: String = "filled"
    }

    override fun describe(): String = "Rect(${width}x$height)"
}

class Circle(private val radius: Double) : Shape {
    override val area: Double
        get() = Pi * radius * radius

    companion object Factory {
        const val DEFAULT_RADIUS = 1.0

        fun unit(): Circle = Circle(DEFAULT_RADIUS)
    }
}

enum class Color(val rgb: Int) {
    RED(0xFF0000),
    GREEN(0x00FF00),
    BLUE(0x0000FF)
}

/** Un companion sin nombre: Kotlin lo llama `Companion` por convenio. */
class Plain {
    companion object {
        fun make(): Plain = Plain()
    }
}

object Registry {
    private val shapes = mutableListOf<Shape>()

    fun register(shape: Shape) {
        shapes.add(shape)
    }
}

val DEFAULT_SHAPE: Shape = Rect(1.0, 1.0)

fun largest(a: Double, b: Double): Double = max(a, b)

fun String.shout(): String = this.uppercase() + "!"

fun overload(x: Int): String = "int"

fun overload(x: String): String = "string"

sealed class Node

value class Money(val amount: Long)
