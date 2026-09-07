using System.Linq;
using TCalc.Domain;
using TCalc.Storage;
using TCalcStorage.Storage;
using TCBlazor.Server.Telegram;
using Xunit;

namespace TCalcTests
{
    /// <summary>Reaching a past spending, and fixing its amount or its wording.</summary>
    public class TourcalcBotSpendingListTests
    {
        private const long Chat = -1001234567;

        private static TgUpdate Msg(string text)
            => new TgUpdate { ChatId = Chat, UserId = 1, DisplayName = "Дима", UserName = "Дима", Text = text };

        private static TgUpdate Tap(string data)
            => new TgUpdate { ChatId = Chat, UserId = 1, DisplayName = "Дима", CallbackData = data, CallbackMessageId = 5 };

        private static TgUpdate Answer(string prompt, string text)
            => new TgUpdate { ChatId = Chat, UserId = 1, DisplayName = "Дима", Text = text, ReplyToBotText = prompt };

        private static Tour Tour(ITourStorage storage)
            => storage.GetTours(t => true, false, 0, int.MaxValue, out _).Single(TgMeta.IsActive);

        private static (TourcalcBot bot, ITourStorage storage) Trip()
        {
            var storage = new InMemoryTourStorage();
            var bot = new TourcalcBot(storage, new TourStorageProcessor(), new TelegramBotOptions());
            bot.Handle(Msg("/newtrip Черногория"));
            bot.Handle(Msg("/add Маша"));
            return (bot, storage);
        }

        private static Spending Only(ITourStorage storage)
            => Tour(storage).Spendings.Single(sp => !sp.Planned);

        [Fact]
        public void AnEmptyListSaysHowToStart()
        {
            var (bot, _) = Trip();
            Assert.Contains("/spend", bot.Handle(Msg("/spendings")).Text);
        }

        [Fact]
        public void EverySpendingIsAButtonThatOpensItsCard()
        {
            // without this a spending was reachable only in the moment it was created
            var (bot, storage) = Trip();
            bot.Handle(Msg("/spend 1200 такси"));
            var id = Only(storage).GUID;

            var list = bot.Handle(Msg("/spendings"));
            var button = list.Buttons.SelectMany(r => r).Single();
            Assert.Equal($"sp:{id}", button.Data);
            Assert.Contains("такси", button.Label);
            Assert.Contains("1 200", button.Label);

            Assert.Contains("такси", bot.Handle(Tap(button.Data)).Text);
        }

        [Fact]
        public void TheListIsNewestFirstAndCapped()
        {
            var (bot, storage) = Trip();
            for (var i = 1; i <= 18; i++) bot.Handle(Msg($"/spend {i}00 трата{i}"));

            var list = bot.Handle(Msg("/spendings"));
            Assert.Equal(15, list.Buttons.Count);            // a keyboard has to stay tappable
            Assert.Contains("из 18", list.Text);
        }

        [Fact]
        public void TheAmountCanBeFixed()
        {
            // the likeliest mistake of all: an extra zero
            var (bot, storage) = Trip();
            bot.Handle(Msg("/spend 12000 ужин"));
            var id = Only(storage).GUID;

            var prompt = bot.Handle(Tap($"amt:{id}"));
            Assert.True(prompt.ForceReply);
            Assert.Contains("12 000", prompt.Text);          // says what it is now

            bot.Handle(Answer(prompt.Text, "1200"));
            Assert.Equal(1200, Only(storage).AmountInCents);
        }

        [Fact]
        public void ANonsenseAmountIsRefusedAndTheOldOneKept()
        {
            var (bot, storage) = Trip();
            bot.Handle(Msg("/spend 1200 ужин"));
            var id = Only(storage).GUID;

            foreach (var bad in new[] { "много", "-5", "0" })
            {
                var reply = bot.Handle(Answer(bot.Handle(Tap($"amt:{id}")).Text, bad));
                Assert.Contains("больше нуля", reply.Text);
            }
            Assert.Equal(1200, Only(storage).AmountInCents);
        }

        [Fact]
        public void TheDescriptionCanBeFixed()
        {
            var (bot, storage) = Trip();
            bot.Handle(Msg("/spend 1200 ужын"));
            var id = Only(storage).GUID;

            var prompt = bot.Handle(Tap($"desc:{id}"));
            Assert.True(prompt.ForceReply);
            bot.Handle(Answer(prompt.Text, "ужин"));

            Assert.Equal("ужин", Only(storage).Description);
        }

        [Fact]
        public void AnEmptyDescriptionIsRefused()
        {
            var (bot, storage) = Trip();
            bot.Handle(Msg("/spend 1200 ужин"));
            var id = Only(storage).GUID;

            var reply = bot.Handle(Answer(bot.Handle(Tap($"desc:{id}")).Text, "   "));
            Assert.Contains("не может быть пустым", reply.Text);
            Assert.Equal("ужин", Only(storage).Description);
        }

        [Fact]
        public void FixingTheAmountChangesWhatPeopleOwe()
        {
            var (bot, storage) = Trip();
            bot.Handle(Msg("/spend 10000 ужин"));
            var id = Only(storage).GUID;
            Assert.Contains("Маша — отдать 5 000", bot.Handle(Msg("/balance")).Text);

            bot.Handle(Answer(bot.Handle(Tap($"amt:{id}")).Text, "1000"));
            Assert.Contains("Маша — отдать 500", bot.Handle(Msg("/balance")).Text);
        }

        [Fact]
        public void EditingSomethingAlreadyDeletedSaysSoInsteadOfThrowing()
        {
            var (bot, _) = Trip();
            Assert.Contains("не найдена", bot.Handle(Tap("amt:nosuch")).CallbackToast);
            Assert.Contains("не найдена", bot.Handle(Tap("desc:nosuch")).CallbackToast);
        }
    }
}
