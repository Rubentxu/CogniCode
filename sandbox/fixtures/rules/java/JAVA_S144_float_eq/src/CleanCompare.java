// Clean: Using epsilon comparison
public class FloatCompare {
    private static final double EPSILON = 0.000001;

    public boolean equals(double a, double b) {
        return Math.abs(a - b) < EPSILON;
    }
}
