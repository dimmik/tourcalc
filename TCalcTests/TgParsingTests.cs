using System.Linq;
using TCBlazor.Server.Telegram;
using Xunit;

namespace TCalcTests
{
    public class TgParsingTests
    {
        [Fact]
        public void NonCommandsAreNotCommands()
        {
            Assert.Null(TgCommand.Parse(null));
            Assert.Null(TgCommand.Parse(""));
            Assert.Null(TgCommand.Parse("просто сообщение"));
        }

        [Fact]
        public void BotNameSuffixIsStripped()
        {
            // Telegram appends it in groups; without stripping, every command in a chat
            // with more than one bot would look unknown
            Assert.True(TgCommand.Parse("/newtrip@tourcalc_test Черногория").Is("newtrip"));
            Assert.Equal("Черногория", TgCommand.Parse("/newtrip@tourcalc_test Черногория").Args);
        }

        [Fact]
        public void MentionsAreSplitOutOfTheArguments()
        {
            var c = TgCommand.Parse("/spend 1200 такси @masha @petya");
            Assert.Equal(new[] { "masha", "petya" }, c.Mentions.ToArray());
            Assert.Equal("1200 такси", c.ArgsWithoutMentions);
        }

        [Theory]
        [InlineData("Олег 35", "Олег", 35)]
        [InlineData("Олег", "Олег", 100)]              // default share
        [InlineData("Дядя Вова", "Дядя Вова", 100)]    // trailing word is not a number
        [InlineData("Олег 0", "Олег", 0)]              // pays for nothing
        [InlineData("  Аня   50  ", "Аня", 50)]
        public void DependentIsNameThenOptionalShare(string input, string name, int weight)
        {
            Assert.True(TourcalcBot.TryParseDependent(input, out var n, out var w));
            Assert.Equal(name, n);
            Assert.Equal(weight, w);
        }

        [Fact]
        public void EmptyDependentIsRejected()
        {
            Assert.False(TourcalcBot.TryParseDependent("   ", out _, out _));
        }

        [Fact]
        public void AllowedChatsAreParsedAndEmptyMeansEveryone()
        {
            Assert.Empty(TelegramBotOptions.ParseChats(""));
            Assert.Equal(new long[] { -100123, 456 }, TelegramBotOptions.ParseChats("-100123; 456").ToArray());
            Assert.Empty(TelegramBotOptions.ParseChats("not-a-number"));

            var open = new TelegramBotOptions();
            Assert.True(open.ChatAllowed(42));

            var limited = new TelegramBotOptions { AllowedChats = new long[] { 1 } };
            Assert.True(limited.ChatAllowed(1));
            Assert.False(limited.ChatAllowed(2));
        }

        [Theory]
        [InlineData("https://tc.dimmik.org/goto/A/b", true)]
        [InlineData("http://tc.dimmik.org/goto/A/b", true)]
        [InlineData("http://localhost:5399/goto/A/b", false)]   // Telegram: "Wrong HTTP URL"
        [InlineData("http://127.0.0.1:5399/goto/A/b", false)]
        [InlineData("http://devbox:5399/goto/A/b", false)]      // no dot, not a public host
        [InlineData("ftp://tc.dimmik.org/x", false)]
        [InlineData("not a url", false)]
        public void OnlyAPublicAddressCanGoOnAButton(string url, bool ok)
        {
            // Telegram rejects the entire message when a button URL displeases it, so this
            // is asked before the button is built rather than discovered by the reply
            // never arriving
            Assert.Equal(ok, TourcalcBot.CanLinkTo(url));
        }

        [Fact]
        public void TheMiniAppEntryIsExcludedFromTheOfflineFallback()
        {
            // the service worker answers every navigation with the app shell unless told
            // otherwise, which is how /t broke; /tgapp is server-rendered the same way
            var sw = System.IO.File.ReadAllText(ServiceWorkerPath());
            Assert.Contains("serverRenderedPaths", sw);
            Assert.Contains("tgapp", sw);
        }

        private static string ServiceWorkerPath()
        {
            var dir = new System.IO.DirectoryInfo(System.AppContext.BaseDirectory);
            while (dir != null && !System.IO.Directory.Exists(System.IO.Path.Combine(dir.FullName, "TCBlazor")))
            {
                dir = dir.Parent;
            }
            Assert.NotNull(dir);
            return System.IO.Path.Combine(dir.FullName, "TCBlazor", "Client", "wwwroot", "service-worker.published.js");
        }

        [Fact]
        public void BotWillNotRunWithoutAToken()
        {
            Assert.False(new TelegramBotOptions { Mode = "polling" }.Enabled);
            Assert.False(new TelegramBotOptions { Token = "x", Mode = "off" }.Enabled);
            Assert.True(new TelegramBotOptions { Token = "x", Mode = "polling" }.Enabled);
        }

        [Fact]
        public void TheSwitchTurnsTheBotOffWithoutDisturbingTheRest()
        {
            var configured = new TelegramBotOptions
            {
                Token = "x", Mode = "webhook", WebhookSecret = "s", EnabledSetting = false,
            };

            Assert.False(configured.Enabled);
            // everything else is still there, so flipping it back needs no remembering
            Assert.Equal("webhook", configured.Mode);
            Assert.Equal("s", configured.WebhookSecret);

            configured.EnabledSetting = true;
            Assert.True(configured.Enabled);
        }

        [Fact]
        public void TheStartupLineSaysWhyTheBotIsOff()
        {
            Assert.Contains("TelegramBot_Enabled",
                new TelegramBotOptions { Token = "x", Mode = "polling", EnabledSetting = false }.DisabledBecause);
            Assert.Contains("TelegramBot_Token",
                new TelegramBotOptions { Mode = "polling" }.DisabledBecause);
            Assert.Contains("TelegramBot_Mode",
                new TelegramBotOptions { Token = "x", Mode = "off" }.DisabledBecause);
            Assert.Null(new TelegramBotOptions { Token = "x", Mode = "polling" }.DisabledBecause);
        }
    }
}
