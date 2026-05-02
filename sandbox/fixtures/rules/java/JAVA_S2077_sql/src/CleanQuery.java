// Clean: Using PreparedStatement
public class UserDAO {
    public User findUser(String id) {
        String query = "SELECT * FROM users WHERE id = ?";
        PreparedStatement stmt = connection.prepareStatement(query);
        stmt.setString(1, id);
        // Execute query...
        return null;
    }
}
