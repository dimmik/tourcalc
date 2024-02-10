using System;
using System.Collections.Generic;
using System.Collections.Specialized;
using System.Text;

namespace TCalcCore.UI
{
    public interface INotifier
    {
        void Notify(string tourId, string message);
    }
    public class DumbNotifier : INotifier
    {
        public void Notify(string tourId, string message)
        {
            // do nothing
        }
    }
}
