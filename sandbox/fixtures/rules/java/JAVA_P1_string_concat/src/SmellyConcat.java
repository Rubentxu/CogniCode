// Smelly: String concatenation in loop
public class StringBuilder {
    public String buildMessage(String[] parts) {
        String result = "";
        for (String part : parts) {
            result += part; // String concat
        }
        return result;
    }
}
