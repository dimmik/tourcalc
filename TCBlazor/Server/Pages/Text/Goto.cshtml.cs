using Microsoft.AspNetCore.Mvc;
using Microsoft.AspNetCore.Mvc.RazorPages;
using Company.TCBlazor;
using Company.TCBlazor.Auth;

namespace TCBlazor.Server.Pages.Text
{
    /// <summary>
    /// The text counterpart of the app's share link: /t/goto/{code md5}/{tour} signs the
    /// reader in and drops them on the tour. Same shape as the /goto the SPA handles, so
    /// a link that is already in someone's hands works by adding the /t prefix.
    /// </summary>
    public class GotoModel : PageModel
    {
        private readonly ITcConfiguration configuration;
        private readonly IECDsaCryptoKey signerKey;

        public GotoModel(ITcConfiguration configuration, IECDsaCryptoKey signerKey)
        {
            this.configuration = configuration;
            this.signerKey = signerKey;
        }

        public IActionResult OnGet(string md5, string tourId)
        {
            var auth = AccessAuthorizer.Authorize(configuration, "code", md5 ?? "", accessCodeIsMd5: true);
            var token = AccessAuthorizer.CreateToken(configuration, signerKey, "code", auth);
            TextAuth.SetCookie(Response, token, AccessAuthorizer.TokenLifetime(configuration));
            return Redirect(string.IsNullOrWhiteSpace(tourId) ? "/t" : $"/t/{tourId}");
        }
    }
}
