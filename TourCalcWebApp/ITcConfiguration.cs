using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;

namespace TourCalcWebApp
{
    public interface ITcConfiguration
    {
        T GetValue<T>(string name, T defVal);
        T GetValue<T>(string name);
    }
}
