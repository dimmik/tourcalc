using System;
using System.Collections.Generic;
using System.Linq;
using System.Security.Cryptography;
using System.Text;
using System.Web;

namespace TCBlazor.Server.Telegram
{
    /// <summary>
    /// Checks that an "initData" string really came from Telegram.
    ///
    /// A Mini App is an ordinary web page, so anything it tells the server about who is
    /// looking at it is worth exactly nothing until this has passed: without the check
    /// anyone could open the page by hand and claim to be any user at all. Telegram signs
    /// the payload with a key derived from the bot token, which only the server knows.
    /// </summary>
    public static class TgWebAppAuth
    {
        /// <summary>What Telegram vouched for: null when the signature did not check out.</summary>
        public class Verified
        {
            public long UserId { get; set; }
            public string UserName { get; set; } = "";
            public DateTimeOffset AuthDate { get; set; }
        }

        /// <summary>
        /// Validates the payload and returns who it is about, or null.
        /// <paramref name="maxAge"/> guards against an old capture being replayed.
        /// </summary>
        public static Verified Validate(string initData, string botToken, TimeSpan maxAge, DateTimeOffset now)
        {
            if (string.IsNullOrWhiteSpace(initData) || string.IsNullOrWhiteSpace(botToken)) return null;

            var pairs = Parse(initData);
            if (!pairs.TryGetValue("hash", out var provided) || string.IsNullOrEmpty(provided)) return null;

            // everything but the hash, sorted by key, one "key=value" per line
            var check = string.Join("\n", pairs
                .Where(p => p.Key != "hash")
                .OrderBy(p => p.Key, StringComparer.Ordinal)
                .Select(p => $"{p.Key}={p.Value}"));

            var secret = Hmac(Encoding.UTF8.GetBytes("WebAppData"), botToken);
            var expected = ToHex(Hmac(secret, check));

            // fixed-time comparison: a hash is a secret being guessed, and a plain string
            // comparison leaks how much of a guess was right
            if (!CryptographicOperations.FixedTimeEquals(
                    Encoding.ASCII.GetBytes(expected),
                    Encoding.ASCII.GetBytes(provided.ToLowerInvariant())))
            {
                return null;
            }

            if (!pairs.TryGetValue("auth_date", out var authDateRaw)
                || !long.TryParse(authDateRaw, out var authDateUnix))
            {
                return null;
            }
            var authDate = DateTimeOffset.FromUnixTimeSeconds(authDateUnix);
            if (now - authDate > maxAge) return null;

            if (!pairs.TryGetValue("user", out var userJson)) return null;
            var user = Newtonsoft.Json.Linq.JObject.Parse(userJson);
            var id = (long?)user["id"];
            if (id == null) return null;

            return new Verified
            {
                UserId = id.Value,
                UserName = (string)user["username"] ?? "",
                AuthDate = authDate,
            };
        }

        private static Dictionary<string, string> Parse(string initData)
        {
            var result = new Dictionary<string, string>(StringComparer.Ordinal);
            foreach (var part in initData.Split('&'))
            {
                var eq = part.IndexOf('=');
                if (eq <= 0) continue;
                var key = HttpUtility.UrlDecode(part.Substring(0, eq));
                var value = HttpUtility.UrlDecode(part.Substring(eq + 1));
                result[key] = value;
            }
            return result;
        }

        private static byte[] Hmac(byte[] key, string message)
        {
            using (var hmac = new HMACSHA256(key))
            {
                return hmac.ComputeHash(Encoding.UTF8.GetBytes(message));
            }
        }

        private static string ToHex(byte[] bytes)
            => string.Concat(bytes.Select(b => b.ToString("x2")));
    }
}
