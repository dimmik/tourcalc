using System;
using Microsoft.AspNetCore.Http;

namespace Company.TCBlazor.Auth
{
    /// <summary>
    /// The cookie the no-JavaScript text pages are signed in with, and the name of the
    /// authentication scheme that reads it. It holds the ordinary Tourcalc token; a
    /// text browser simply has no way to put one in an Authorization header.
    /// </summary>
    public static class TextAuth
    {
        public const string Scheme = "TextCookie";
        public const string CookieName = "tc_text";

        public static void SetCookie(HttpResponse response, string token, TimeSpan lifetime)
        {
            response.Cookies.Append(CookieName, token, new CookieOptions
            {
                // no script may read it, and it is never sent to another site: the text
                // pages change data through plain form posts, so SameSite=Strict costs
                // nothing here and closes the cross-site request the cookie would enable
                HttpOnly = true,
                SameSite = SameSiteMode.Strict,
                // the app is served over http in development and https in production;
                // marking it Secure unconditionally would drop it in the former
                Secure = false,
                IsEssential = true,
                Path = "/",
                Expires = DateTimeOffset.UtcNow.Add(lifetime),
            });
        }

        public static void ClearCookie(HttpResponse response)
        {
            response.Cookies.Delete(CookieName, new CookieOptions { Path = "/" });
        }
    }
}
