package hello;

/**
 * A simple greeting class for CogniCode sandbox testing.
 */
public class Greeter {

    /**
     * Returns a greeting string for the given name.
     *
     * @param name the name to greet
     * @return the greeting string
     */
    public String greet(String name) {
        return "Hello, " + name + "!";
    }

    /**
     * Adds two integers.
     *
     * @param a first operand
     * @param b second operand
     * @return the sum
     */
    public int add(int a, int b) {
        return a + b;
    }

    /**
     * Subtracts b from a.
     *
     * @param a first operand
     * @param b second operand
     * @return the difference
     */
    public int subtract(int a, int b) {
        return a - b;
    }

    /**
     * Multiplies two integers (used for mutation testing only).
     *
     * @param a first operand
     * @param b second operand
     * @return the product
     */
    public int multiply(int a, int b) {
        return a * b;
    }

    public static void main(String[] args) {
        Greeter g = new Greeter();
        System.out.println(g.greet("CogniCode"));
        System.out.println("2 + 3 = " + g.add(2, 3));
    }
}
