using System;
using System.Threading;
using System.Threading.Tasks;
using Telegram.Bot;
using Telegram.Bot.Types;

namespace TCBlazor.Server.Telegram
{
    /// <summary>
    /// One update, start to finish: translate it, ask the bot, send whatever it decided.
    /// Both the webhook and the polling loop go through here, so the two ways of
    /// receiving updates cannot behave differently.
    /// </summary>
    public class TgDispatcher
    {
        private readonly TourcalcBot bot;
        private readonly ITelegramBotClient client;

        public TgDispatcher(TourcalcBot bot, ITelegramBotClient client)
        {
            this.bot = bot;
            this.client = client;
        }

        public async Task Dispatch(Update update, CancellationToken token)
        {
            var incoming = TgTransport.ToTgUpdate(update);
            if (incoming == null) return;

            TgReply reply;
            try
            {
                reply = bot.Handle(incoming);
            }
            catch (Exception e)
            {
                // one bad update must not stop the bot; the chat gets told something went wrong
                reply = TgReply.Say("Что-то пошло не так. Попробуй ещё раз.");
                Console.Error.WriteLine($"Telegram bot: {e}");
            }

            try
            {
                await TgTransport.Send(client, incoming.ChatId, reply, update.CallbackQuery?.Id, token);
            }
            catch (Exception e)
            {
                // Deliberately swallowed. By this point the update has already been acted
                // on - a trip created, a person added - and letting the failure out would
                // make the webhook answer with an error, on which Telegram redelivers the
                // same update and the change happens a second time. A lost reply is worth
                // less than a duplicated trip.
                Console.Error.WriteLine($"Telegram bot: could not send the reply: {e.Message}");
            }
        }
    }
}
