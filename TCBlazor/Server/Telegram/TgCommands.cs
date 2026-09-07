using System.Collections.Generic;
using System.Linq;

namespace TCBlazor.Server.Telegram
{
    /// <summary>
    /// The commands the bot answers, in one place.
    ///
    /// Both the menu Telegram shows after "/" and the text of /help are built from this,
    /// because they were drifting apart: each was edited by hand whenever a command was
    /// added, and the menu additionally had to be pushed to Telegram by somebody
    /// remembering to do it.
    /// </summary>
    public static class TgCommands
    {
        public class Entry
        {
            public Entry(string name, string arguments, string description)
            {
                Name = name;
                Arguments = arguments;
                Description = description;
            }

            /// <summary>Without the slash, as Telegram wants it.</summary>
            public string Name { get; }

            /// <summary>Shown in /help only - Telegram's menu has no room for it.</summary>
            public string Arguments { get; }

            public string Description { get; }
        }

        public static readonly IReadOnlyList<Entry> All = new[]
        {
            new Entry("newtrip",   "<название>",        "завести поездку"),
            new Entry("spend",     "<сумма> <на что>",  "записать трату"),
            new Entry("spendings", "",                  "список трат, чтобы поправить старую"),
            new Entry("balance",   "",                  "кто сколько должен"),
            new Entry("settle",    "",                  "кто кому платит"),
            new Entry("who",       "",                  "кто едет"),
            new Entry("add",       "<Имя> [доля]",      "добавить человека без телеграма"),
            new Entry("covers",    "<Имя> [доля]",      "записать того, кого ты везёшь"),
            new Entry("trips",     "",                  "поездки этого чата"),
            new Entry("use",       "<название>",        "переключиться на другую поездку"),
            new Entry("link",      "",                  "открыть в браузере"),
            new Entry("help",      "",                  "что умею"),
        };

        /// <summary>The lines /help prints, one per command.</summary>
        public static string HelpLines()
            => string.Join("\n", All.Select(c => string.IsNullOrEmpty(c.Arguments)
                ? $"/{c.Name} — {c.Description}"
                : $"/{c.Name} {c.Arguments} — {c.Description}"));
    }
}
