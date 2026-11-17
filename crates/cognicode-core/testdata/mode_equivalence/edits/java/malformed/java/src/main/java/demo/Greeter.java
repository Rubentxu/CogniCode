package demo;

public class Greeter {
    private final String prefix;

    public Greeter(String prefix {
        this.prefix = prefix;

    public String greet(String name) {
        return prefix + " " + name

    public String shout(String name) {
        return greet(name).toUpperCase(;
    }
