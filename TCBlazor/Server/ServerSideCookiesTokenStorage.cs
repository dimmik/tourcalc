using TCalcCore.Auth;

namespace TCBlazor.Server
{
    public class ServerSideCookiesTokenStorage : ITokenStorage
    {
        private readonly IHttpContextAccessor httpContextAccessor;

        public ServerSideCookiesTokenStorage(IHttpContextAccessor httpContextAccessor)
        {
            this.httpContextAccessor = httpContextAccessor ?? throw new ArgumentNullException(nameof(httpContextAccessor));
        }
        private string TokenCookieName = "__tc_token";
        public Task<string> GetToken()
        {
            HttpContext? context = httpContextAccessor.HttpContext;
            if (context != null)
            {
                var tokenCookie = context.Request.Cookies.ContainsKey(TokenCookieName) ? context.Request.Cookies[TokenCookieName] : "";
                return Task.FromResult(tokenCookie ?? string.Empty);
            }
            return Task.FromResult(string.Empty);
        }

        public Task SetToken(string token)
        {
            // nothing
            return Task.CompletedTask;
        }
    }
}
