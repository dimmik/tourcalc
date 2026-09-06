using System;
using Microsoft.AspNetCore.Mvc;
using Microsoft.AspNetCore.Mvc.RazorPages;
using TCalc.Storage;
using Company.TCBlazor;
using Company.TCBlazor.Auth;

namespace TCBlazor.Server.Pages.Text
{
    /// <summary>
    /// The one place a text browser can sign in. It hands the code to the same
    /// authorizer the JSON API uses and keeps the resulting token in a cookie.
    /// </summary>
    public class LoginModel : PageModel
    {
        private readonly ITcConfiguration configuration;
        private readonly IECDsaCryptoKey signerKey;

        public LoginModel(ITcConfiguration configuration, IECDsaCryptoKey signerKey)
        {
            this.configuration = configuration;
            this.signerKey = signerKey;
        }

        public bool Failed { get; private set; }

        [BindProperty]
        public string Code { get; set; } = "";

        [BindProperty]
        public string Scope { get; set; } = "code";

        public void OnGet()
        {
        }

        public IActionResult OnPost([FromQuery] string next)
        {
            var scope = Scope == "admin" ? "admin" : "code";
            try
            {
                var auth = AccessAuthorizer.Authorize(configuration, scope, Code ?? "");
                var token = AccessAuthorizer.CreateToken(configuration, signerKey, scope, auth);
                TextAuth.SetCookie(Response, token, AccessAuthorizer.TokenLifetime(configuration));
            }
            catch
            {
                // a wrong master key throws; a wrong access code does not - it simply
                // matches no tours, and the empty list says so
                Failed = true;
                return Page();
            }
            return Redirect(SafeNext(next));
        }

        /// <summary>
        /// Only ever come back to a path on this site: "next" arrives in the query string,
        /// so without this the login form would forward a reader to anywhere at all.
        /// </summary>
        internal static string SafeNext(string next)
        {
            if (string.IsNullOrWhiteSpace(next)) return "/t";
            if (!next.StartsWith("/") || next.StartsWith("//")) return "/t";
            return next;
        }
    }
}
