// Smelly: Throw in finally block
public class Handler {
    public void handle() {
        Connection conn = null;
        try {
            conn = getConnection();
            doWork(conn);
        } finally {
            if (conn != null) {
                conn.close();  // cleanup
            }
            throw new RuntimeException("cleanup failed");  // This masks original exception!
        }
    }

    private Connection getConnection() { return null; }
    private void doWork(Connection c) {}
}

class Connection {
    void close() {}
}
