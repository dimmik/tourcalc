using Microsoft.JSInterop;
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
        public async Task SetObject<T>(string key, T obj) where T : new()
        {
            string json = JsonSerializer.Serialize(obj ?? new());
            await Set(key, json);
        }
        public async Task<T> GetObject<T>(string key) where T : new()
        {
            string json = await Get(key);
            T res = JsonSerializer.Deserialize<T>(json ?? "") ?? new T();
            return res;
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
            var settings = JsonSerializer.Deserialize<UISettings>(res) ?? new UISettings();
            return settings;
        }
        public async Task SetUISettings(UISettings s)
        {
            await Set(UISettingsKey, JsonSerializer.Serialize(s));
        }
    }
}
