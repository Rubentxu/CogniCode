namespace Acme.Core;

public record Price(decimal Amount, string Currency);

public struct Point
{
    public int X;
    public int Y;
}

public partial class Ledger
{
    public decimal Total() => 0m;
}

public class Calculator
{
    public int Add(int a, int b) => a + b;

    public double Add(double a, double b) => a + b;
}
