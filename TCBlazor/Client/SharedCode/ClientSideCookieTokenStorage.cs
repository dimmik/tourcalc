using TCalcCore.Auth;
using Microsoft.JSInterop;
using TCalcCore.Logging;

namespace TCBlazor.Client.SharedCode
{
    public class ClientSideCookieTokenStorage : ITokenStorage
    {

        private readonly IJSRuntime JS;
        private readonly ILocalLogger logger;
        public ClientSideCookieTokenStorage(IJSRuntime js, ILocalLogger logger)
        {
            JS = js;
            this.logger = logger;
        }

        private readonly string TokenCookieName = "__tc_token";

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
