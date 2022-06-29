using System;
using System.Threading.Tasks;
using TCalcCore.UI;

namespace TCalcCore.Storage
{
    public interface ITourcalcLocalStorage
    {
        Task<(string val, DateTimeOffset stored)> Get(string key);
        Task<(T val, DateTimeOffset stored)> GetObject<T>(string key);
        Task<(string val, DateTimeOffset stored)> GetToken();
        Task<(UISettings val, DateTimeOffset stored)> GetUISettings();
        Task Set(string key, string val);
        Task SetObject<T>(string key, T obj);
        Task SetToken(string token);
        Task SetUISettings(UISettings s);
    }
}