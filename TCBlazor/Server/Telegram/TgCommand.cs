using System;
using System.Collections.Generic;
using System.Linq;

namespace TCBlazor.Server.Telegram
{
    /// <summary>A parsed "/command arguments" line.</summary>
    public class TgCommand
    {
        public string Name { get; private set; } = "";
        public string Args { get; private set; } = "";

        /// <summary>Words of <see cref="Args"/> that begin with "@", without the "@".</summary>
        public IReadOnlyList<string> Mentions { get; private set; } = new List<string>();

        /// <summary>Args with the mentions taken out - the human part.</summary>
        public string ArgsWithoutMentions { get; private set; } = "";

        public bool Is(string name) => string.Equals(Name, name, StringComparison.OrdinalIgnoreCase);

        /// <summary>
        /// Parses a message into a command, or returns null when it is not one.
        ///
        /// In a group Telegram appends the bot's own name - "/spend@tourcalc_bot" - and a
        /// message that keeps it would otherwise look like an unknown command whenever
        /// several bots share a chat.
        /// </summary>
        public static TgCommand Parse(string text)
        {
            if (string.IsNullOrWhiteSpace(text)) return null;
            var line = text.Trim();
            if (!line.StartsWith("/")) return null;

            var space = line.IndexOf(' ');
            var head = space < 0 ? line : line.Substring(0, space);
            var rest = space < 0 ? "" : line.Substring(space + 1).Trim();

            var at = head.IndexOf('@');
            if (at > 0) head = head.Substring(0, at);

            var mentions = rest
                .Split(new[] { ' ', '\t', '\n' }, StringSplitOptions.RemoveEmptyEntries)
                .Where(w => w.StartsWith("@") && w.Length > 1)
                .Select(w => w.Substring(1))
                .ToList();

            var without = string.Join(" ", rest
                .Split(new[] { ' ', '\t', '\n' }, StringSplitOptions.RemoveEmptyEntries)
                .Where(w => !w.StartsWith("@")));

            return new TgCommand
            {
                Name = head.Substring(1),
                Args = rest,
                Mentions = mentions,
                ArgsWithoutMentions = without,
            };
        }
    }
}
