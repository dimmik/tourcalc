using System;
using System.Linq;
using System.Text.RegularExpressions;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.Extensions.Hosting;
using Telegram.Bot;
using Telegram.Bot.Types.Enums;

namespace TCBlazor.Server.Telegram
{
    /// <summary>
    /// Tells Telegram where to deliver updates, on startup, when running in webhook mode.
    ///
    /// It is the mirror of what the polling service does when it removes the webhook, and
    /// it exists so a deployment is self-contained: set the variables, deploy, done. Doing
    /// it by hand worked exactly until the next time somebody forgot.
    /// </summary>
    public class TelegramWebhookRegistrar : BackgroundService
    {
        private readonly TelegramBotOptions options;
        private readonly ITelegramBotClient client;

        public TelegramWebhookRegistrar(TelegramBotOptions options, ITelegramBotClient client)
        {
            this.options = options;
            this.client = client;
        }

        /// <summary>
        /// Why Telegram would refuse this configuration, or null when it would not. Checked
        /// before asking, so the reason ends up in the log at startup rather than as a bot
        /// that mysteriously never hears anything.
        /// </summary>
        internal static string Problem(TelegramBotOptions options)
        {
            if (!Uri.TryCreate(options.WebhookUrl, UriKind.Absolute, out var uri))
            {
                return $"TelegramBot_PublicBaseUrl is not a valid address ({options.PublicBaseUrl})";
            }
            // Telegram delivers webhooks over HTTPS only
            if (uri.Scheme != Uri.UriSchemeHttps)
            {
                return $"a webhook must be https, and this is {uri.Scheme} ({options.WebhookUrl}). " +
                       "Use TelegramBot_Mode=polling for local work.";
            }
            if (string.IsNullOrEmpty(options.WebhookSecret))
            {
                // the controller refuses every request without it, so registering would
                // create a webhook whose every delivery is rejected
                return "TelegramBot_WebhookSecret is empty, and the endpoint rejects anything without it";
            }
            // Telegram's own rule for the secret header
            if (!Regex.IsMatch(options.WebhookSecret, "^[A-Za-z0-9_-]{1,256}$"))
            {
                return "TelegramBot_WebhookSecret may only contain A-Z, a-z, 0-9, _ and -, up to 256 characters";
            }
            return null;
        }

        protected override async Task ExecuteAsync(CancellationToken stoppingToken)
        {
            if (!options.IsWebhook) return;

            var problem = Problem(options);
            if (problem != null)
            {
                Console.Error.WriteLine($"Telegram bot: not registering the webhook - {problem}");
                return;
            }

            try
            {
                var current = await client.GetWebhookInfoAsync(stoppingToken);
                if (current?.Url == options.WebhookUrl)
                {
                    // re-registering the same address would drop nothing but achieves nothing
                    Console.WriteLine($"Telegram bot: webhook already set to {options.WebhookUrl}");
                    return;
                }

                await client.SetWebhookAsync(
                    url: options.WebhookUrl,
                    secretToken: options.WebhookSecret,
                    allowedUpdates: new[] { UpdateType.Message, UpdateType.CallbackQuery },
                    cancellationToken: stoppingToken);

                Console.WriteLine($"Telegram bot: webhook set to {options.WebhookUrl}");
            }
            catch (Exception e)
            {
                // the app must still serve the site if Telegram is unreachable at boot
                Console.Error.WriteLine($"Telegram bot: could not set the webhook: {e.Message}");
            }
        }
    }
}
