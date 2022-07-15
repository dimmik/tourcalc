using TCalcCore.Auth;
using Microsoft.JSInterop;
using TCalcCore.Logging;

namespace TCBlazor.Client.Shared
{
    public class CookieTokenStorage : ITokenStorage
    {

        private readonly IJSRuntime JS;
        private readonly ILocalLogger logger;
        public CookieTokenStorage(IJSRuntime js, ILocalLogger logger)
        {
            JS = js;
            this.logger = logger;
        }

        private string TokenCookieName = "__tc_token";

        public async Task<string> GetToken()
        {
            var s = await JS.InvokeAsync<string>("getCookie", TokenCookieName);
            return s ?? string.Empty;
        }

        public async Task SetToken(string token)
        {
            // function setCookie(name, value, days) {
            await JS.InvokeVoidAsync("setCookie", TokenCookieName, token, 365);
        }
    }
}
