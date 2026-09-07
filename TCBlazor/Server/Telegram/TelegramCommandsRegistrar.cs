using System;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.Extensions.Hosting;
using Telegram.Bot;
using Telegram.Bot.Types;

namespace TCBlazor.Server.Telegram
{
    /// <summary>
    /// Publishes the command menu - what Telegram offers after "/" - at startup.
    ///
    /// It used to be pushed by hand with a curl, which meant the test bot and the
    /// production one drifted apart and a newly added command was missing from the menu
    /// until somebody noticed. The list lives in TgCommands, so deploying the code is now
    /// the same act as publishing the menu.
    /// </summary>
    public class TelegramCommandsRegistrar : BackgroundService
    {
        private readonly TelegramBotOptions options;
        private readonly ITelegramBotClient client;

        public TelegramCommandsRegistrar(TelegramBotOptions options, ITelegramBotClient client)
        {
            this.options = options;
            this.client = client;
        }

        protected override async Task ExecuteAsync(CancellationToken stoppingToken)
        {
            if (!options.Enabled) return;

            var wanted = TgCommands.All
                .Select(c => new BotCommand { Command = c.Name, Description = c.Description })
                .ToArray();

            try
            {
                var current = await client.GetMyCommandsAsync(cancellationToken: stoppingToken);
                if (Same(current, wanted))
                {
                    return;
                }

                await client.SetMyCommandsAsync(wanted, cancellationToken: stoppingToken);
                Console.WriteLine($"Telegram bot: menu published, {wanted.Length} commands");
            }
            catch (Exception e)
            {
                // the menu is a convenience; the commands themselves work without it, so
                // an unreachable Telegram at boot must not take the app down with it
                Console.Error.WriteLine($"Telegram bot: could not publish the menu: {e.Message}");
            }
        }

        /// <summary>Nothing to say to Telegram when it already has exactly this list.</summary>
        internal static bool Same(BotCommand[] current, BotCommand[] wanted)
        {
            if (current == null) return false;
            if (current.Length != wanted.Length) return false;
            return current.Zip(wanted, (a, b) => a.Command == b.Command && a.Description == b.Description).All(x => x);
        }
    }
}
