using System.Collections.Generic;

namespace TCBlazor.Server.Telegram
{
    /// <summary>
    /// One thing that happened in Telegram, stripped of Telegram's own types.
    ///
    /// The bot's logic is written against this and never sees Telegram.Bot, so it can be
    /// exercised without a token, a network or a bot at all - which is most of why the
    /// transport is kept as thin as it is.
    /// </summary>
    public class TgUpdate
    {
        public long ChatId { get; set; }
        public bool IsPrivate { get; set; }
        public string ChatTitle { get; set; } = "";

        public long UserId { get; set; }
        public string UserName { get; set; } = "";
        public string DisplayName { get; set; } = "";

        /// <summary>Message text, if this was a message.</summary>
        public string Text { get; set; }

        /// <summary>Payload of a tapped button, if this was one.</summary>
        public string CallbackData { get; set; }

        /// <summary>Id of the message a button was tapped on - the one to edit in place.</summary>
        public int? CallbackMessageId { get; set; }

        /// <summary>Text of the bot message this one replies to, if any. Carries the prompt.</summary>
        public string ReplyToBotText { get; set; }
    }

    public class TgButton
    {
        public TgButton(string label, string data)
        {
            Label = label;
            Data = data;
        }

        public string Label { get; }
        public string Data { get; }
    }

    /// <summary>What the bot wants done. Null means "say nothing".</summary>
    public class TgReply
    {
        public string Text { get; set; } = "";

        /// <summary>Rows of buttons; each row is a list.</summary>
        public List<List<TgButton>> Buttons { get; set; } = new List<List<TgButton>>();

        /// <summary>Edit this message instead of posting a new one - keeps the chat quiet.</summary>
        public int? EditMessageId { get; set; }

        /// <summary>Ask Telegram to open a reply box: the only way to collect free text
        /// from a group without turning the bot's privacy mode off.</summary>
        public bool ForceReply { get; set; }

        /// <summary>Short toast shown on the tapping user's screen only.</summary>
        public string CallbackToast { get; set; }

        public TgReply Row(params TgButton[] buttons)
        {
            Buttons.Add(new List<TgButton>(buttons));
            return this;
        }

        public static TgReply Say(string text) => new TgReply { Text = text };

        public static TgReply Toast(string text) => new TgReply { CallbackToast = text };
    }
}
