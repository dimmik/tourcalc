﻿using Microsoft.JSInterop;

namespace TCBlazor.Client.Storage
{
    public class TourcalcLocalStorage
    {
        private IJSRuntime JS;
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
    }
}