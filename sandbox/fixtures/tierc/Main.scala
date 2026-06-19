// Scala test fixture
object Main {
  def compute(x: Int): Int = x * 2

  def greet(name: String): Unit = println(s"Hello, $name")

  def main(args: Array[String]): Unit = {
    val result = compute(42)
    greet("world")
    println(result)
  }
}
