// Smelly: Dead store - x is assigned but never used
public class DeadStore {
    public int compute() {
        int x = 5;
        return 42;
    }
}
