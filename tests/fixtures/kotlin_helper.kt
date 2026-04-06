/**
 * A documented Kotlin utility class.
 *
 * @param prefix the greeting prefix
 */
class KotlinHelper(val prefix: String) {

    /**
     * Format a greeting.
     *
     * @param name the recipient name
     * @return formatted string
     */
    fun greet(name: String): String = "$prefix $name"

    object Companion {
        val DEFAULT_PREFIX = "Hello"
    }
}
