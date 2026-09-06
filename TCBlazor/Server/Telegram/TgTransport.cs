using System.Collections.Generic;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using Telegram.Bot;
using Telegram.Bot.Types;
using Telegram.Bot.Types.Enums;
using Telegram.Bot.Types.ReplyMarkups;

namespace TCBlazor.Server.Telegram
{
    /// <summary>
    /// The only place that knows Telegram's own types: it turns an Update into the bot's
    /// <see cref="TgUpdate"/>, and the bot's <see cref="TgReply"/> back into API calls.
    /// Everything else is transport-free, which is what lets the logic be tested without
    /// a token.
    /// </summary>
    public static class TgTransport
    {
        /// <summary>Telegram's shape, reduced to what the bot actually uses. Null if uninteresting.</summary>
        public static TgUpdate ToTgUpdate(Update update)
        {
            if (update?.CallbackQuery is { } callback)
            {
                var from = callback.From;
                var chat = callback.Message?.Chat;
                if (chat == null) return null;
                return new TgUpdate
                {
                    ChatId = chat.Id,
                    IsPrivate = chat.Type == ChatType.Private,
                    ChatTitle = chat.Title ?? "",
                    UserId = from.Id,
                    UserName = from.Username ?? "",
                    DisplayName = DisplayName(from),
                    CallbackData = callback.Data,
                    CallbackMessageId = callback.Message.MessageId,
                };
            }

            var message = update?.Message;
            if (message?.From == null || string.IsNullOrEmpty(message.Text)) return null;

            return new TgUpdate
            {
                ChatId = message.Chat.Id,
                IsPrivate = message.Chat.Type == ChatType.Private,
                ChatTitle = message.Chat.Title ?? "",
                UserId = message.From.Id,
                UserName = message.From.Username ?? "",
                DisplayName = DisplayName(message.From),
                Text = message.Text,
                // only the bot's own text matters: it carries the prompt a reply answers
                ReplyToBotText = message.ReplyToMessage?.From?.IsBot == true
                    ? message.ReplyToMessage.Text
                    : null,
            };
        }

        private static string DisplayName(User user)
        {
            var name = string.Join(" ", new[] { user.FirstName, user.LastName }
                .Where(p => !string.IsNullOrWhiteSpace(p)));
            return string.IsNullOrWhiteSpace(name) ? (user.Username ?? "") : name;
        }

        public static async Task Send(ITelegramBotClient client, long chatId, TgReply reply,
                                      string callbackQueryId, CancellationToken token)
        {
            if (reply == null) return;

            if (!string.IsNullOrEmpty(reply.CallbackToast) && callbackQueryId != null)
            {
                await client.AnswerCallbackQueryAsync(callbackQueryId, reply.CallbackToast, cancellationToken: token);
                return;
            }

            // a tapped button leaves a spinner on the client until it is answered
            if (callbackQueryId != null)
            {
                await client.AnswerCallbackQueryAsync(callbackQueryId, cancellationToken: token);
            }

            if (string.IsNullOrEmpty(reply.Text)) return;

            var markup = Markup(reply);

            if (reply.EditMessageId.HasValue)
            {
                await client.EditMessageTextAsync(chatId, reply.EditMessageId.Value, reply.Text,
                    replyMarkup: markup as InlineKeyboardMarkup, cancellationToken: token);
                return;
            }

            await client.SendTextMessageAsync(chatId, reply.Text, replyMarkup: markup, cancellationToken: token);
        }

        private static IReplyMarkup Markup(TgReply reply)
        {
            if (reply.ForceReply) return new ForceReplyMarkup { Selective = true };
            if (!reply.Buttons.Any()) return null;
            return new InlineKeyboardMarkup(reply.Buttons
                .Select(row => (IEnumerable<InlineKeyboardButton>)row
                    .Select(b => InlineKeyboardButton.WithCallbackData(b.Label, b.Data))
                    .ToList()));
        }
    }
}
