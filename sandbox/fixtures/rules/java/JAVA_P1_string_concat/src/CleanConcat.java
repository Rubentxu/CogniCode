// Clean: StringBuilder for concatenation
public class StringBuilder {
    public String buildMessage(String[] parts) {
        StringBuilder result = new StringBuilder();
        for (String part : parts) {
            result.append(part);
        }
        return result.toString();
    }
}
