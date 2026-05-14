// Clean: No throw in finally
public class Handler {
    public void handle() {
        Connection conn = null;
        try {
            conn = getConnection();
            doWork(conn);
        } finally {
            if (conn != null) {
                conn.close();  // cleanup only
            }
            // No throw here - original exception propagates
        }
    }

    private Connection getConnection() { return null; }
    private void doWork(Connection c) {}
}

class Connection {
    void close() {}
}
