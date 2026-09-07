using System.Linq;
using TCBlazor.Server.Telegram;
using Telegram.Bot.Types;
using Xunit;

namespace TCalcTests
{
    public class TgCommandsTests
    {
        [Fact]
        public void EveryAdvertisedCommandIsOneTheBotActuallyAnswers()
        {
            // the menu promising something the bot ignores is worse than no menu
            var source = System.IO.File.ReadAllText(BotSourcePath());
            foreach (var command in TgCommands.All.Where(c => c.Name != "help"))
            {
                Assert.Contains($"command.Is(\"{command.Name}\")", source);
            }
            Assert.Contains("command.Is(\"help\")", source);
        }

        [Fact]
        public void HelpListsEveryCommand()
        {
            var help = TgCommands.HelpLines();
            foreach (var command in TgCommands.All)
            {
                Assert.Contains($"/{command.Name}", help);
                Assert.Contains(command.Description, help);
            }
        }

        [Theory]
        [InlineData("newtrip")]
        [InlineData("spendings")]
        public void ArgumentsAreShownInHelpButNotInTheName(string name)
        {
            var entry = TgCommands.All.Single(c => c.Name == name);
            Assert.DoesNotContain(" ", entry.Name);          // Telegram would refuse it
            Assert.Contains($"/{name}", TgCommands.HelpLines());
        }

        [Fact]
        public void NamesAndDescriptionsFitWhatTelegramAccepts()
        {
            foreach (var command in TgCommands.All)
            {
                Assert.Matches("^[a-z0-9_]{1,32}$", command.Name);
                Assert.InRange(command.Description.Length, 3, 256);
            }
        }

        [Fact]
        public void NothingIsSaidToTelegramWhenTheMenuIsAlreadyRight()
        {
            var wanted = TgCommands.All
                .Select(c => new BotCommand { Command = c.Name, Description = c.Description })
                .ToArray();

            Assert.True(TelegramCommandsRegistrar.Same(wanted, wanted));
            Assert.False(TelegramCommandsRegistrar.Same(null, wanted));
            Assert.False(TelegramCommandsRegistrar.Same(wanted.Take(2).ToArray(), wanted));

            var changed = wanted.Select(c => new BotCommand { Command = c.Command, Description = c.Description }).ToArray();
            changed[0].Description = "что-то другое";
            Assert.False(TelegramCommandsRegistrar.Same(changed, wanted));
        }

        private static string BotSourcePath()
        {
            var dir = new System.IO.DirectoryInfo(System.AppContext.BaseDirectory);
            while (dir != null && !System.IO.Directory.Exists(System.IO.Path.Combine(dir.FullName, "TCBlazor")))
            {
                dir = dir.Parent;
            }
            Assert.NotNull(dir);
            return System.IO.Path.Combine(dir.FullName, "TCBlazor", "Server", "Telegram", "TourcalcBot.cs");
        }
    }
}
