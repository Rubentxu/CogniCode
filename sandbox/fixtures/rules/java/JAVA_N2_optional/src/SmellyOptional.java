// Smelly: Optional.get() without isPresent check
import java.util.Optional;

public class OptionalHandler {
    public String getValue(Optional<String> opt) {
        return ((Optional<String>)opt).get();
    }
}
