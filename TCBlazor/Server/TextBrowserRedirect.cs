using System;
using System.Linq;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Http;
using Microsoft.Extensions.DependencyInjection;
using Company.TCBlazor;

namespace TCBlazor.Server
{
    /// <summary>
    /// Sends a known text browser to the text interface instead of the app shell it
    /// cannot run.
    ///
    /// This is a convenience, not the mechanism: the honest signal is the &lt;noscript&gt;
    /// block in index.html, which keys off the actual missing capability and so covers
    /// every browser with scripting off, including ones nobody has heard of. Matching on
    /// the user agent only recognises names it has been told about, and such a list ages
    /// - which is why it is configuration rather than code, and why it is small.
    /// </summary>
    public static class TextBrowserRedirect
    {
        /// <summary>Config key: which agents count as text browsers, separated by ";".</summary>
        public const string AgentsKey = "TextBrowserAgents";

        /// <summary>Config key: set false to stop redirecting altogether.</summary>
        public const string EnabledKey = "TextBrowserRedirectEnabled";

        /// <summary>
        /// "elinks" is not here on purpose: "links" is a substring of it, so the shorter
        /// name already catches ELinks.
        /// </summary>
        public const string DefaultAgents = "lynx;w3m;links";

        public static IApplicationBuilder UseTextBrowserRedirect(this IApplicationBuilder app)
        {
            var configuration = app.ApplicationServices.GetService<ITcConfiguration>();

            // Read once, at startup. ITcConfiguration.GetValue captures a stack trace on
            // every call for its diagnostics page, which is fine for configuration read
            // when something is being set up and quite wrong to do per request. The cost
            // is that changing the list needs a restart - acceptable for a list that
            // changes when a new text browser appears.
            var enabled = configuration?.GetValue(EnabledKey, true) ?? true;
            if (!enabled) return app;

            var agents = ParseAgents(configuration?.GetValue(AgentsKey, DefaultAgents) ?? DefaultAgents);
            if (agents.Length == 0) return app;

            return app.Use(async (context, next) =>
            {
                if (TryMap(context, agents, out var target))
                {
                    // temporary: whether a reader wants the text pages is not something
                    // to write into their cache for good
                    context.Response.Redirect(target, permanent: false);
                    return;
                }
                await next();
            });
        }

        internal static string[] ParseAgents(string configured)
            => (configured ?? "")
                .Split(new[] { ';', ',' }, StringSplitOptions.RemoveEmptyEntries)
                .Select(a => a.Trim())
                .Where(a => a.Length > 0)
                .ToArray();

        private static bool TryMap(HttpContext context, string[] agents, out string target)
        {
            target = null;
            var request = context.Request;

            if (!HttpMethods.IsGet(request.Method)) return false;
            if (!IsTextBrowser(request.Headers["User-Agent"].ToString(), agents)) return false;

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

        private static bool IsTextBrowser(string userAgent, string[] agents)
        {
            if (string.IsNullOrEmpty(userAgent)) return false;
            return agents.Any(b => userAgent.Contains(b, StringComparison.OrdinalIgnoreCase));
        }
    }
}
