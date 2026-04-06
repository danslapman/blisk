package example

import scala.collection.mutable.ListBuffer
import scala.util.Try

// See https://example.com/docs for more info
class Animal(val name: String, var age: Int) {
  def speak(): String = s"I am $name"
  val sound = "..."
}

object Animal {
  def apply(name: String): Animal = new Animal(name, 0)
}

trait Describable {
  def describe(): String
}
