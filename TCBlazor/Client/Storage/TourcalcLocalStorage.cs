using Microsoft.JSInterop;

namespace TCBlazor.Client.Storage
{
    public class TourcalcLocalStorage
    {
        private IJSRuntime JS;
        public readonly static string TokenKey = "__tc_token";
        public TourcalcLocalStorage(IJSRuntime js)
        {
            JS = js;
        }
        public async Task<string> Get(string key)
        {
            var res = await JS.InvokeAsync<string>("localStorage.getItem", new object[] { key });
            return res;
        }
        public async Task Set(string key, string val)
        {
            await JS.InvokeVoidAsync("localStorage.setItem", new object[] { key, val });
        }
        public async Task<string> GetToken()
        {
            return await Get(TokenKey);
        }
        public async Task SetToken(string token)
        {
            await Set(TokenKey, token);
        }
        private Dictionary<string, object?> pageLocalStorage = new Dictionary<string, object?>();
        public void SetPageLocalValue(string key, object? val)
        {
            pageLocalStorage[key] = val;
        }
        public object? GetPageLocalValue(string key)
        {
            if (pageLocalStorage.ContainsKey(key))
            {
                return pageLocalStorage[key];
            }
            return null;
        }
    }
}
