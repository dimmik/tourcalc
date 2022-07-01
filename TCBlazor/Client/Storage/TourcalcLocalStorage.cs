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
        private readonly ILocalLogger logger;
        public TourcalcLocalStorage(IJSRuntime js, ILocalLogger logger)
        {
            JS = js;
            this.logger = logger;
        }
        private static string GetStoredDatetimeKey(string key) => $"{key}_stored_datetime";

        public async Task<(string val, DateTimeOffset stored)> Get(string key, string defVal = "", bool storeDefaultValue = false)
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
            var val = res;
            if (res == null)
            {
                val = defVal;
                if (storeDefaultValue)
                {
                    await Set(key, val);
                }
            }
            return (val, st);
        }
        public async Task Set(string key, string val)
        {
            DateTimeOffset st = DateTimeOffset.Now;
            string stored = Newtonsoft.Json.JsonConvert.SerializeObject(st);
            await JS.InvokeVoidAsync("localStorage.setItem", new object[] { key, val });
            await JS.InvokeVoidAsync("localStorage.setItem", new object[] { GetStoredDatetimeKey(key), stored });
        }
    }
}
