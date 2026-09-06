using System;
using Microsoft.AspNetCore.Authentication;
using Microsoft.AspNetCore.Builder;
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

        /// <summary>
        /// Signs the reader in from the cookie for requests under /t, and only those.
        ///
        /// It has to happen in middleware rather than inside the page: antiforgery ties
        /// its token to the identity it was issued for, and the filter that checks it runs
        /// before any page code. Authenticating in the handler meant the form was rendered
        /// for a signed-in reader and its token then checked against an anonymous one, so
        /// every post was rejected. Scoping it by path keeps the JSON API exactly as it
        /// was: it never sees this, so a cookie still cannot speak for the reader there.
        /// </summary>
        public static IApplicationBuilder UseTextAuth(this IApplicationBuilder app)
            => app.Use(async (context, next) =>
            {
                if (context.Request.Path.StartsWithSegments("/t"))
                {
                    var result = await context.AuthenticateAsync(Scheme);
                    if (result?.Principal != null) context.User = result.Principal;
                }
                await next();
            });

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
