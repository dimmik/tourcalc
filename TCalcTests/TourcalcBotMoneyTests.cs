using System.Linq;
using TCalc.Domain;
using TCalc.Storage;
using TCalcStorage.Storage;
using TCBlazor.Server.Telegram;
using Xunit;

namespace TCalcTests
{
    /// <summary>Spending, balances and settling up - still with no Telegram anywhere.</summary>
    public class TourcalcBotMoneyTests
    {
        private const long Chat = -1001234567;

        private static (TourcalcBot bot, ITourStorage storage) NewBot()
        {
            var storage = new InMemoryTourStorage();
            return (new TourcalcBot(storage, new TourStorageProcessor(),
                new TelegramBotOptions { PublicBaseUrl = "https://tc.example.org" }), storage);
        }

        private static TgUpdate Msg(string text, long user = 1, string name = "Дима")
            => new TgUpdate { ChatId = Chat, UserId = user, DisplayName = name, UserName = name, Text = text };

        private static TgUpdate Tap(string data, long user = 1)
            => new TgUpdate { ChatId = Chat, UserId = user, DisplayName = "Дима", CallbackData = data, CallbackMessageId = 5 };

        private static Tour Tour(ITourStorage storage)
            => storage.GetTours(t => true, false, 0, int.MaxValue, out _).Single(TgMeta.IsActive);

        private static (TourcalcBot bot, ITourStorage storage) TripWithTwoPeople()
        {
            var (bot, storage) = NewBot();
            bot.Handle(Msg("/newtrip Черногория"));   // Дима joins as its author
            bot.Handle(Msg("/add Маша"));
            return (bot, storage);
        }

        [Theory]
        [InlineData("1200 такси из аэропорта", 1200, "такси из аэропорта")]
        [InlineData("50 кофе", 50, "кофе")]
        public void SpendingIsAnAmountThenWhatItWasFor(string input, long amount, string what)
        {
            Assert.True(TourcalcBot.TryParseSpending(input, out var a, out var w));
            Assert.Equal(amount, a);
            Assert.Equal(what, w);
        }

        [Theory]
        [InlineData("такси 1200")]   // the other way round
        [InlineData("1200")]         // no description
        [InlineData("-5 кофе")]      // not a positive amount
        [InlineData("")]
        public void NonsenseSpendingsAreRejected(string input)
            => Assert.False(TourcalcBot.TryParseSpending(input, out _, out _));

        [Fact]
        public void AddedPeopleHaveNoTelegramAccountAndSettleForThemselves()
        {
            var (_, storage) = TripWithTwoPeople();
            var masha = Tour(storage).Persons.Single(p => p.Name == "Маша");

            Assert.False(TgMeta.IsTelegramPerson(masha));
            Assert.Null(masha.ParentId);           // unlike /covers, she is nobody's dependent
            Assert.Equal(100, masha.Weight);
        }

        [Fact]
        public void ASpendingIsFiledUnderACategoryOrItWouldNotCountAsSpending()
        {
            // an empty category means "money handed over to settle a debt" to the app,
            // and the expense would vanish from every total
            var (bot, storage) = TripWithTwoPeople();
            bot.Handle(Msg("/spend 1200 такси"));

            var spending = Tour(storage).Spendings.Single(sp => !sp.Planned);
            Assert.Equal(TourcalcBot.DefaultCategory, spending.Type);
            Assert.False(string.IsNullOrWhiteSpace(spending.Type));
            Assert.True(spending.ToAll);
            Assert.Equal(1200, spending.AmountInCents);
        }

        [Fact]
        public void TheSenderPaysUnlessSaidOtherwise()
        {
            var (bot, storage) = TripWithTwoPeople();
            bot.Handle(Msg("/spend 1200 такси"));

            var tour = Tour(storage);
            var dima = tour.Persons.Single(p => p.Name == "Дима");
            Assert.Equal(dima.GUID, tour.Spendings.Single(sp => !sp.Planned).FromGuid);
        }

        [Fact]
        public void ThePayerCanBeChangedToSomebodyWithNoTelegramAccount()
        {
            var (bot, storage) = TripWithTwoPeople();
            bot.Handle(Msg("/spend 1200 такси"));
            var spendingId = Tour(storage).Spendings.Single(sp => !sp.Planned).GUID;
            var masha = Tour(storage).Persons.Single(p => p.Name == "Маша");

            var picker = bot.Handle(Tap($"from:{spendingId}"));
            Assert.Contains("Кто платил?", picker.Text);
            Assert.Contains(picker.Buttons.SelectMany(r => r), b => b.Label == "Маша");

            var card = bot.Handle(Tap($"from:{spendingId}:{masha.GUID}"));
            Assert.Contains("платил Маша", card.Text);
            Assert.Equal(masha.GUID, Tour(storage).Spendings.Single(sp => !sp.Planned).FromGuid);
        }

        [Fact]
        public void ASpendingCanBeDeleted()
        {
            var (bot, storage) = TripWithTwoPeople();
            bot.Handle(Msg("/spend 1200 такси"));
            var spendingId = Tour(storage).Spendings.Single(sp => !sp.Planned).GUID;

            var reply = bot.Handle(Tap($"del:{spendingId}"));
            Assert.Contains("удалено", reply.Text);
            Assert.DoesNotContain(Tour(storage).Spendings, sp => sp.GUID == spendingId);
        }

        [Fact]
        public void BalanceSaysWhoOwesAndWhoGetsBack()
        {
            var (bot, storage) = TripWithTwoPeople();
            bot.Handle(Msg("/spend 1000 ужин"));      // Дима paid for both

            var reply = bot.Handle(Msg("/balance"));
            Assert.Contains("Потрачено 1 000", reply.Text);
            Assert.Contains("Дима — получить 500", reply.Text);
            Assert.Contains("Маша — отдать 500", reply.Text);
        }

        [Fact]
        public void BalanceShowsDependentsUnderWhoeverCarriesThem()
        {
            var (bot, _) = TripWithTwoPeople();
            bot.Handle(Msg("/covers Олег 35"));

            Assert.Contains("└ Олег — через Дима", bot.Handle(Msg("/balance")).Text);
        }

        [Fact]
        public void SettleListsThePaymentsAndMarkingOneRecordsIt()
        {
            var (bot, storage) = TripWithTwoPeople();
            bot.Handle(Msg("/spend 1000 ужин"));

            var settle = bot.Handle(Msg("/settle"));
            Assert.Contains("Маша → Дима", settle.Text);

            var button = settle.Buttons.SelectMany(r => r).Single();
            Assert.StartsWith("paid:", button.Data);

            var after = bot.Handle(Tap(button.Data));
            Assert.Contains("Все в расчёте", after.Text);

            // the payment is written down as a real spending with no category, which moves
            // the balances without counting as money spent on the trip
            var recorded = Tour(storage).Spendings.Where(sp => !sp.Planned).ToList();
            Assert.Equal(2, recorded.Count);
            Assert.Contains(recorded, sp => string.IsNullOrEmpty(sp.Type));
        }

        [Fact]
        public void NothingToSettleWhenNobodySpent()
            => Assert.Contains("Все в расчёте", TripWithTwoPeople().bot.Handle(Msg("/settle")).Text);

        [Fact]
        public void SpendingNeedsYouToBeOnTheTrip()
        {
            var (bot, storage) = TripWithTwoPeople();
            var reply = bot.Handle(Msg("/spend 100 кофе", user: 99, name: "Чужой"));

            Assert.Contains("Я еду", reply.Text);
            Assert.DoesNotContain(Tour(storage).Spendings, sp => !sp.Planned);
        }
    }
}
