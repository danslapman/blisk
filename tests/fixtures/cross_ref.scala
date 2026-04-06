class Greeter {
  val greeting = "Hello"

  def greet(name: String): String = {
    val message = greeting + ", " + name
    message
  }

  def greetAll(names: List[String]): List[String] = {
    names.map(n => greeting + " " + n)
  }
}
