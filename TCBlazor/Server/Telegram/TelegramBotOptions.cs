using System;
using System.Collections.Generic;
using System.Linq;
using Company.TCBlazor;

namespace TCBlazor.Server.Telegram
{
    /// <summary>How the bot is configured. Read once at startup, not per update.</summary>
    public class TelegramBotOptions
    {
        public const string TokenKey = "TelegramBot_Token";
        public const string ModeKey = "TelegramBot_Mode";
        public const string WebhookSecretKey = "TelegramBot_WebhookSecret";
        public const string AllowedChatsKey = "TelegramBot_AllowedChats";
        public const string BaseUrlKey = "TelegramBot_PublicBaseUrl";

        public string Token { get; set; } = "";

        /// <summary>off | polling | webhook. Polling is for local work: it needs no public address.</summary>
        public string Mode { get; set; } = "off";

        public string WebhookSecret { get; set; } = "";

        /// <summary>Empty means any chat may use the bot.</summary>
        public IReadOnlyList<long> AllowedChats { get; set; } = new List<long>();

        public string PublicBaseUrl { get; set; } = "https://tc.dimmik.org";

        public bool IsPolling => string.Equals(Mode, "polling", StringComparison.OrdinalIgnoreCase);
        public bool IsWebhook => string.Equals(Mode, "webhook", StringComparison.OrdinalIgnoreCase);

        /// <summary>A bot with no token cannot run whatever the mode says.</summary>
        public bool Enabled => !string.IsNullOrWhiteSpace(Token) && (IsPolling || IsWebhook);

        public bool ChatAllowed(long chatId) => AllowedChats.Count == 0 || AllowedChats.Contains(chatId);

        public static TelegramBotOptions Read(ITcConfiguration configuration)
        {
            var options = new TelegramBotOptions
            {
                Token = configuration.GetValue(TokenKey, ""),
                Mode = configuration.GetValue(ModeKey, "off"),
                WebhookSecret = configuration.GetValue(WebhookSecretKey, ""),
                PublicBaseUrl = configuration.GetValue(BaseUrlKey, "https://tc.dimmik.org"),
                AllowedChats = ParseChats(configuration.GetValue(AllowedChatsKey, "")),
            };
            return options;
        }

        internal static IReadOnlyList<long> ParseChats(string configured)
            => (configured ?? "")
                .Split(new[] { ';', ',', ' ' }, StringSplitOptions.RemoveEmptyEntries)
                .Select(c => long.TryParse(c.Trim(), out var id) ? (long?)id : null)
                .Where(id => id.HasValue)
                .Select(id => id.Value)
                .ToList();
    }
}
