using Acme.Core;

namespace Acme.App;

public static class Program
{
    public static void Main()
    {
        var calc = new Calculator();
        System.Console.WriteLine(calc.Add(1, 2).ToString().Shout());
    }
}
