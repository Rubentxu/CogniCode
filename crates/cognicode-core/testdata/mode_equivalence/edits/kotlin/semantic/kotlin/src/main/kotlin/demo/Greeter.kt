package demo

class Greeter(private val prefix: String) {
    fun greet(name: String): String = "$prefix $name"

    fun yell(name: String): String = greet(name).uppercase()
}

fun total(values: List<Long>): Long = values.sum()

fun average(values: List<Long>): Long = total(values) / values.size
