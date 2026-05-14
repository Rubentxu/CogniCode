// Clean: Catching specific exception
import java.io.IOException;

public class Handler {
    public void handle() {
        try {
            doSomething();
        } catch (IOException e) {
            System.err.println("IO error: " + e.getMessage());
        }
    }

    private void doSomething() throws IOException {}
}
