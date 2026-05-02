// Clean: Test with assertions
import org.junit.Test;
import static org.junit.Assert.*;

public class CleanTest {
    @Test
    public void testSomething() {
        int result = 1 + 1;
        assertEquals(2, result);
    }
}
