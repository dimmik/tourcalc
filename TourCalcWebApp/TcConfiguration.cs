using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using Microsoft.Extensions.Configuration;

namespace TourCalcWebApp
{
    public class TcConfiguration : ITcConfiguration
    {
        private readonly IConfiguration Configuration;
        public TcConfiguration(IConfiguration config)
        {
            Configuration = config;
        }
        public T GetValue<T>(string name, T defVal)
        {
            return Configuration.GetValue(name, defVal);
        }

        public T GetValue<T>(string name)
        {
            return Configuration.GetValue<T>(name);
        }
    }
}
