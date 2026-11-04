namespace Demo
{
    public class Repo<T>
    {
        public T Item { get; set; }
    }

    public class IntBox : Repo<int>
    {
    }
}
