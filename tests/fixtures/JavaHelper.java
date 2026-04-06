/**
 * A documented Java utility class.
 *
 * @author blisk-test
 */
public class JavaHelper {
    /** The default prefix. */
    private String prefix;

    /**
     * Format a greeting.
     *
     * @param name the recipient name
     * @return formatted greeting string
     */
    public String greet(String name) {
        return prefix + name;
    }

    public enum Status { ACTIVE, INACTIVE }
}
