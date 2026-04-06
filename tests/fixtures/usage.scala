import example.Animal

object Main {
  def run(): Unit = {
    val a = new Animal("Cat", 3)
    val greeter = new Greeter()
    greeter.greet("World")
  }
}
