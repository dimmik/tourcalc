using System;
using System.Linq;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Http;

namespace TCBlazor.Server
{
    /// <summary>
    /// Sends a known text browser to the text interface instead of the app shell it
    /// cannot run.
    ///
    /// This is a convenience, not the mechanism: the honest signal is the &lt;noscript&gt;
    /// block in index.html, which keys off the actual missing capability and so covers
    /// every browser with scripting off, including ones nobody has heard of. Matching
    /// on the user agent only recognises the names below, and that list will age - it
    /// exists so the common case needs no click, and it is deliberately kept small.
    /// </summary>
    public static class TextBrowserRedirect
    {
        private static readonly string[] TextBrowsers = { "lynx", "w3m", "elinks", "links" };

        public static IApplicationBuilder UseTextBrowserRedirect(this IApplicationBuilder app)
            => app.Use(async (context, next) =>
            {
                if (TryMap(context, out var target))
                {
                    // temporary: whether a reader wants the text pages is not something
                    // to write into their cache for good
                    context.Response.Redirect(target, permanent: false);
                    return;
                }
                await next();
            });

        private static bool TryMap(HttpContext context, out string target)
        {
            target = null;
            var request = context.Request;

            if (!HttpMethods.IsGet(request.Method)) return false;
            if (!IsTextBrowser(request.Headers["User-Agent"].ToString())) return false;

            var path = request.Path.HasValue ? request.Path.Value : "/";

            // already there, or not ours to touch
            if (path == "/t" || path.StartsWith("/t/", StringComparison.Ordinal)) return false;
            if (path.StartsWith("/api", StringComparison.Ordinal)) return false;
            if (path.StartsWith("/_", StringComparison.Ordinal)) return false;

            var parts = path.Split('/', StringSplitOptions.RemoveEmptyEntries);

            if (parts.Length == 0 || parts[0] == "tourlist")
            {
                target = "/t";
                return true;
            }

            // /goto/{code md5}/{tour} is the share link, and it carries the sign-in
            if (parts[0] == "goto" && parts.Length >= 2)
            {
                target = "/t/goto/" + string.Join("/", parts.Skip(1));
                return true;
            }

            // /tour/{id}[/persons|/spendings] - the app's own per-section routes have
            // text counterparts, so a link into one lands on the matching page
            if (parts[0] == "tour" && parts.Length >= 2)
            {
                var section = parts.Length >= 3 ? parts[2] : "";
                var suffix = section switch
                {
                    "persons" => "/people",
                    "spendings" => "/spend",
                    _ => "",
                };
                target = $"/t/{parts[1]}{suffix}";
                return true;
            }

            // anything else the app would have handled: the tour list is the way in
            target = "/t";
            return true;
        }

        private static bool IsTextBrowser(string userAgent)
        {
            if (string.IsNullOrEmpty(userAgent)) return false;
            return TextBrowsers.Any(b => userAgent.Contains(b, StringComparison.OrdinalIgnoreCase));
        }
    }
}
