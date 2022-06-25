using System.Threading.Tasks;
using TCalcCore.UI;

namespace TCalcCore.Storage
{
    public interface ITourcalcLocalStorage
    {
        Task<string> Get(string key);
        Task<T> GetObject<T>(string key);
        Task<string> GetToken();
        Task<UISettings> GetUISettings();
        Task Set(string key, string val);
        Task SetObject<T>(string key, T obj);
        Task SetToken(string token);
        Task SetUISettings(UISettings s);
    }
}