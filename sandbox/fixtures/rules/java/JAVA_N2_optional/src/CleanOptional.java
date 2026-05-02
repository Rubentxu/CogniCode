// Clean: Using orElse
import java.util.Optional;

public class OptionalHandler {
    public String getValue(Optional<String> opt) {
        return opt.orElse("default");
    }
}
