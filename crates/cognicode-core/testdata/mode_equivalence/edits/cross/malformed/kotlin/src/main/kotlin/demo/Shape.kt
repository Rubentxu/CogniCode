package demo

interface Shape {
    fun area(): Double
}

data class Circle(val radius: Double) : Shape {
    override fun area(): Double = 3.14159 * radius * radius
}

fun unitCircle(): Circle = Circle(1.0
