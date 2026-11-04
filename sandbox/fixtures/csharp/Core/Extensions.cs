namespace Acme.Core;

public static class StringExtensions
{
    public static string Shout(this string text) => text.ToUpperInvariant() + "!";
}
