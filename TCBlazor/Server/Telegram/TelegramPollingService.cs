using System;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.Extensions.Hosting;
using Telegram.Bot;
using Telegram.Bot.Polling;
using Telegram.Bot.Types;
using Telegram.Bot.Types.Enums;

namespace TCBlazor.Server.Telegram
{
    /// <summary>
    /// Long polling, for working on the bot locally: it needs no public address and no
    /// tunnel, so a test bot can talk to a server running on a laptop behind NAT.
    /// Production uses the webhook instead.
    /// </summary>
    public class TelegramPollingService : BackgroundService
    {
        private readonly TelegramBotOptions options;
        private readonly ITelegramBotClient client;
        private readonly TgDispatcher dispatcher;

        public TelegramPollingService(TelegramBotOptions options, ITelegramBotClient client, TgDispatcher dispatcher)
        {
            this.options = options;
            this.client = client;
            this.dispatcher = dispatcher;
        }

        protected override async Task ExecuteAsync(CancellationToken stoppingToken)
        {
            if (!options.IsPolling) return;

            // a webhook left over from another run would stop polling from receiving anything
            await client.DeleteWebhookAsync(dropPendingUpdates: false, cancellationToken: stoppingToken);

            var receiverOptions = new ReceiverOptions
            {
                AllowedUpdates = new[] { UpdateType.Message, UpdateType.CallbackQuery },
            };

            var me = await client.GetMeAsync(stoppingToken);
            Console.WriteLine($"Telegram bot: polling as @{me.Username}");

            await client.ReceiveAsync(
                updateHandler: (_, update, token) => dispatcher.Dispatch(update, token),
                pollingErrorHandler: (_, exception, _) =>
                {
                    Console.Error.WriteLine($"Telegram polling: {exception.Message}");
                    return Task.CompletedTask;
                },
                receiverOptions: receiverOptions,
                cancellationToken: stoppingToken);
        }
    }
}
