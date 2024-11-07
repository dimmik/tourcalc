using Newtonsoft.Json;
using TCalc.Domain;
using TCalcCore.Domain;

namespace TCalcCore.Helpers
{
    internal static class ExtensionMethods
    {
        public static T Clone<T>(this T item) where T : ITourcalcItem
        {
            var json = JsonConvert.SerializeObject(item);
            var clone = JsonConvert.DeserializeObject<T>(json);
            return clone;
        }
    }
}
