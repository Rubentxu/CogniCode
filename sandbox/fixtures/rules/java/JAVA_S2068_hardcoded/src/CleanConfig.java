// Clean: Using environment variable
public class Config {
    public String getPassword() {
        String password = System.getenv("PASS");
        return password;
    }
}
