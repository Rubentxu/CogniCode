package demo

// A comment-only and body-only edit.
class Greeter(private val prefix: String) {
    fun greet(name: String): String = "$prefix, $name"

    fun shout(name: String): String = greet(name).uppercase()
}

fun total(values: List<Long>): Long = values.sum()
