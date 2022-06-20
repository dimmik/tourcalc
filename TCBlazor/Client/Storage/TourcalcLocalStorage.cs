using Microsoft.JSInterop;
using System.Diagnostics;
using System.Text.Json;

namespace TCBlazor.Client.Storage
{
    public class TourcalcLocalStorage
    {
        private IJSRuntime JS;
        public readonly static string TokenKey = "__tc_token";
        public readonly static string UISettingsKey = "__tc_ui_settings";
        public TourcalcLocalStorage(IJSRuntime js)
        {
            JS = js;
        }
        public async Task<string> Get(string key)
        {
            var res = await JS.InvokeAsync<string>("localStorage.getItem", new object[] { key });
            return res ?? "";
        }
        public async Task Set(string key, string val)
        {
            await JS.InvokeVoidAsync("localStorage.setItem", new object[] { key, val });
        }
        public async Task<string> GetToken()
        {
            return await Get(TokenKey);
        }
        public async Task SetObject<T>(string key, T obj)
        {
            Stopwatch sw = Stopwatch.StartNew();
            string json = Newtonsoft.Json.JsonConvert.SerializeObject(obj);
            await Set(key, json);
            sw.Stop();
            Console.WriteLine($"Set object {key} in {sw.Elapsed}");
        }
        public async Task<T?> GetObject<T>(string key)
        {
            Stopwatch sw = Stopwatch.StartNew();
            string json = await Get(key);
            Console.WriteLine($"j: {json}");
            try
            {
                T? res = Newtonsoft.Json.JsonConvert.DeserializeObject<T>(json ?? "");
                sw.Stop();
                Console.WriteLine($"Get object {key} (null? {res == null}) in {sw.Elapsed}");
                return res;
            }
            catch (Exception e)
            {
                Console.WriteLine($"Error get obj: {e.Message}");
                return default;
            }
        }
        public async Task SetToken(string token)
        {
            await Set(TokenKey, token);
        }
        public async Task<UISettings> GetUISettings()
        {
            var res = await Get(UISettingsKey);
            if (string.IsNullOrWhiteSpace(res))
            {
                var s = new UISettings();
                await SetUISettings(s);
                return s;
            }
            var settings = Newtonsoft.Json.JsonConvert.DeserializeObject<UISettings>(res) ?? new UISettings();
            return settings;
        }
        public async Task SetUISettings(UISettings s)
        {
            await Set(UISettingsKey, Newtonsoft.Json.JsonConvert.SerializeObject(s));
        }
    }
}
