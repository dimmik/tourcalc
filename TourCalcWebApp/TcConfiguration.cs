using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Threading.Tasks;
using Microsoft.Extensions.Configuration;

namespace TourCalcWebApp
{
    public class TcConfiguration : ITcConfiguration
    {
        
        private readonly IConfiguration Configuration;
        public readonly Dictionary<string, RequestedConfigValue> RequestedValues = new Dictionary<string, RequestedConfigValue>();
        public TcConfiguration(IConfiguration config)
        {
            Configuration = config;
        }
        private string GetStackTrace()
        {
            StackTrace stackTrace = new StackTrace();

            // Get calling method name
            return $"{stackTrace.GetFrame(2).GetMethod().DeclaringType.Namespace}.{stackTrace.GetFrame(2).GetMethod().DeclaringType.Name}.{stackTrace.GetFrame(2).GetMethod().Name}";
        }
        public T GetValue<T>(string name, T defVal)
        {
            bool isThereValue = Configuration.AsEnumerable().Any(x => x.Key == name);
            var rv = new RequestedConfigValue()
            {
                IsDefaultProvided = true,
                DefaultValue = $"{defVal}",
                Name = name,
                IsDefaultUsed = false,
                CalledFrom = GetStackTrace()
            };
            T val;
            if (!isThereValue)
            {
                rv.IsDefaultUsed = true;
                val = defVal;
            } else
            {
                val = Configuration.GetValue<T>(name);
            }
            RequestedValues[name] = rv;
            return val;
        }

        public T GetValue<T>(string name)
        {
            var val = Configuration.GetValue<T>(name);
            var rv = new RequestedConfigValue()
            {
                IsDefaultProvided = false,
                DefaultValue = null,
                Name = name,
                IsDefaultUsed = false,
                CalledFrom = GetStackTrace()
            };
            RequestedValues[name] = rv;
            return val;
        }
    }
    public class RequestedConfigValue
    {
        public string Name { get; set; }
        public bool IsDefaultProvided { get; set; }
        public string DefaultValue { get; set; }
        public bool IsDefaultUsed { get; set; }
        public string CalledFrom { get; set; }
    }
}
