package example

/** A documented class with a greeting. */
class Greeter {
  /**
   * Says hello to the given name.
   *
   * @param name the person to greet
   * @return a greeting string
   * @see [[Undocumented]]
   */
  def greet(name: String): String = s"Hello, $name"

  val greeting = "Hello"
}

class Undocumented

/**
 * Shows example usage.
 *
 * Example:
 * {{{
 * val g = new Greeter()
 * g.greet("World")
 * }}}
 */
def withExample(): Unit = ()
