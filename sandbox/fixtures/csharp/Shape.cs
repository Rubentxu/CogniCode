using System;

namespace Demo
{
    public abstract class Shape : IShape, IDisposable
    {
        public abstract double Area();

        public void Dispose()
        {
        }
    }
}
