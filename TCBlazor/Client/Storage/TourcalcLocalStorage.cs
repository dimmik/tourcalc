using Microsoft.JSInterop;
using System.Diagnostics;
using TCalcCore.Logging;
using TCalcCore.Storage;
using TCalcCore.UI;
using TCBlazor.Client.Shared;


namespace TCBlazor.Client.Storage
{
    public class TourcalcLocalStorage : ITourcalcLocalStorage
    {
        private readonly IJSRuntime JS;
        public readonly static string TokenKey = "__tc_token";
        public readonly static string UISettingsKey = "__tc_ui_settings";
        private readonly ILocalLogger logger;
        public TourcalcLocalStorage(IJSRuntime js, ILocalLogger logger)
        {
            JS = js;
            this.logger = logger;
        }
        private string GetStoredDatetimeKey(string key)
        {
            return $"{key}_stored_datetime";
        }
        public async Task<(string val, DateTimeOffset stored)> Get(string key)
        {
            var res = await JS.InvokeAsync<string>("localStorage.getItem", new object[] { key });
            var stored = await JS.InvokeAsync<string>("localStorage.getItem", new object[] { GetStoredDatetimeKey(key) });
            DateTimeOffset st = DateTimeOffset.Now;
            try
            {
                st = Newtonsoft.Json.JsonConvert.DeserializeObject<DateTimeOffset>(stored);
            } catch (Exception)
            {
                // nothing
            }
            return (res ?? "", st);
        }
        public async Task Set(string key, string val)
        {
            DateTimeOffset st = DateTimeOffset.Now;
            string stored = Newtonsoft.Json.JsonConvert.SerializeObject(st);
            await JS.InvokeVoidAsync("localStorage.setItem", new object[] { key, val });
            await JS.InvokeVoidAsync("localStorage.setItem", new object[] { GetStoredDatetimeKey(key), stored });
        }
        public async Task<(string val, DateTimeOffset stored)> GetToken()
        {
            return await Get(TokenKey);
        }
        public async Task SetObject<T>(string key, T obj)
        {
            Stopwatch sw = Stopwatch.StartNew();
            string json = Newtonsoft.Json.JsonConvert.SerializeObject(obj);
            await Set(key, json);
            sw.Stop();
            logger.Log($"Set object {key} in {sw.Elapsed}");
        }
        public async Task<(T? val, DateTimeOffset stored)> GetObject<T>(string key)
        {
            Stopwatch sw = Stopwatch.StartNew();
            var (json, dt) = await Get(key);
            //Console.WriteLine($"j: {json}");
            try
            {
                T? res = Newtonsoft.Json.JsonConvert.DeserializeObject<T>(json ?? "");
                sw.Stop();
                logger.Log($"Get object {key} (null? {res == null}) in {sw.Elapsed}");
                return (res, dt);
            }
            catch (Exception e)
            {
                logger.Log($"Error get obj: {e.Message}");
                return (default, dt);
            }
        }
        public async Task SetToken(string token)
        {
            await Set(TokenKey, token);
        }
        public async Task<(UISettings val, DateTimeOffset stored)> GetUISettings()
        {
            var (res, stored) = await Get(UISettingsKey);
            if (string.IsNullOrWhiteSpace(res))
            {
                var s = new UISettings();
                await SetUISettings(s);
                return (s, stored);
            }
            var settings = Newtonsoft.Json.JsonConvert.DeserializeObject<UISettings>(res) ?? new UISettings();
            return (settings, stored);
        }
        public async Task SetUISettings(UISettings s)
        {
            await Set(UISettingsKey, Newtonsoft.Json.JsonConvert.SerializeObject(s));
        }
    }
}
