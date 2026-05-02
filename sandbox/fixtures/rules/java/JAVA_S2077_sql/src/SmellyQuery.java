// Smelly: SQL injection vulnerability
public class UserDAO {
    public User findUser(String table, String id) {
        return statement.executeQuery("SELECT * FROM " + table + " WHERE id = " + id);
    }
}
