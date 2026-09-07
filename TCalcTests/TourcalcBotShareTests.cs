using System.Linq;
using TCalc.Domain;
using TCalc.Storage;
using TCalcStorage.Storage;
using TCBlazor.Server.Telegram;
using Xunit;

namespace TCalcTests
{
    /// <summary>Splitting a spending, categories, and several trips in one chat.</summary>
    public class TourcalcBotShareTests
    {
        private const long Chat = -1001234567;

        private static TgUpdate Msg(string text, long user = 1, string name = "Дима")
            => new TgUpdate { ChatId = Chat, UserId = user, DisplayName = name, UserName = name, Text = text };

        private static TgUpdate Tap(string data)
            => new TgUpdate { ChatId = Chat, UserId = 1, DisplayName = "Дима", CallbackData = data, CallbackMessageId = 5 };

        private static TgUpdate Answer(string prompt, string text)
            => new TgUpdate { ChatId = Chat, UserId = 1, DisplayName = "Дима", Text = text, ReplyToBotText = prompt };

        private static Tour Active(ITourStorage storage)
            => storage.GetTours(t => true, false, 0, int.MaxValue, out _).Single(TgMeta.IsActive);

        private static (TourcalcBot bot, ITourStorage storage) TripWithThree()
        {
            var storage = new InMemoryTourStorage();
            var bot = new TourcalcBot(storage, new TourStorageProcessor(), new TelegramBotOptions());
            bot.Handle(Msg("/newtrip Черногория"));   // Дима
            bot.Handle(Msg("/add Маша"));
            bot.Handle(Msg("/add Петя"));
            bot.Handle(Msg("/spend 900 ужин"));
            return (bot, storage);
        }

        private static Spending TheSpending(ITourStorage storage)
            => Active(storage).Spendings.Single(sp => !sp.Planned);

        [Fact]
        public void ASpendingStartsOnEveryone()
        {
            var (_, storage) = TripWithThree();
            Assert.True(TheSpending(storage).ToAll);
        }

        [Fact]
        public void UntickingSomebodyKeepsTheRestRatherThanEmptyingTheList()
        {
            var (bot, storage) = TripWithThree();
            var id = TheSpending(storage).GUID;
            var petya = Active(storage).Persons.Single(p => p.Name == "Петя");

            bot.Handle(Tap($"to:{id}:{petya.GUID}"));

            var spending = TheSpending(storage);
            Assert.False(spending.ToAll);
            Assert.Equal(2, spending.ToGuid.Count);                 // Дима and Маша remain
            Assert.DoesNotContain(petya.GUID, spending.ToGuid);
        }

        [Fact]
        public void TickingSomebodyBackAddsThem()
        {
            var (bot, storage) = TripWithThree();
            var id = TheSpending(storage).GUID;
            var petya = Active(storage).Persons.Single(p => p.Name == "Петя");

            bot.Handle(Tap($"to:{id}:{petya.GUID}"));
            bot.Handle(Tap($"to:{id}:{petya.GUID}"));

            Assert.Contains(petya.GUID, TheSpending(storage).ToGuid);
        }

        [Fact]
        public void RemovingEverybodyFallsBackToEveryone()
        {
            // a spending for nobody is not something the calculator can split
            var (bot, storage) = TripWithThree();
            var id = TheSpending(storage).GUID;

            foreach (var person in Active(storage).Persons.ToList())
            {
                bot.Handle(Tap($"to:{id}:{person.GUID}"));
            }

            Assert.True(TheSpending(storage).ToAll);
            Assert.Empty(TheSpending(storage).ToGuid);
        }

        [Fact]
        public void ThereIsAWayBackToEveryone()
        {
            var (bot, storage) = TripWithThree();
            var id = TheSpending(storage).GUID;
            var petya = Active(storage).Persons.Single(p => p.Name == "Петя");

            bot.Handle(Tap($"to:{id}:{petya.GUID}"));
            bot.Handle(Tap($"to:{id}:*"));

            Assert.True(TheSpending(storage).ToAll);
        }

        [Fact]
        public void SplittingChangesWhoOwesWhat()
        {
            var (bot, storage) = TripWithThree();
            var id = TheSpending(storage).GUID;
            var petya = Active(storage).Persons.Single(p => p.Name == "Петя");

            Assert.Contains("Петя — отдать 300", bot.Handle(Msg("/balance")).Text);

            bot.Handle(Tap($"to:{id}:{petya.GUID}"));           // the dinner was not his
            Assert.Contains("Петя — в расчёте", bot.Handle(Msg("/balance")).Text);
        }

        [Fact]
        public void TheCategoryCanBePickedFromTheOnesAlreadyUsedOrTypedIn()
        {
            var (bot, storage) = TripWithThree();
            var id = TheSpending(storage).GUID;

            var prompt = bot.Handle(Tap($"cat:{id}:x"));
            Assert.True(prompt.ForceReply);
            bot.Handle(Answer(prompt.Text, "еда"));
            Assert.Equal("еда", TheSpending(storage).Type);

            // now "еда" is one of the tour's categories and can be picked by button
            var picker = bot.Handle(Tap($"cat:{id}"));
            Assert.Contains(picker.Buttons.SelectMany(r => r), b => b.Label == "еда");
        }

        [Fact]
        public void AnEmptyCategoryIsRefusedBecauseItWouldStopBeingASpending()
        {
            var (bot, storage) = TripWithThree();
            var id = TheSpending(storage).GUID;
            var reply = bot.Handle(Answer(bot.Handle(Tap($"cat:{id}:x")).Text, "   "));

            Assert.Contains("не может быть пустой", reply.Text);
            Assert.Equal(TourcalcBot.DefaultCategory, TheSpending(storage).Type);
        }

        [Fact]
        public void OneChatCanHoldSeveralTripsAndSwitchBetweenThem()
        {
            var (bot, storage) = TripWithThree();
            bot.Handle(Msg("/newtrip Грузия"));
            Assert.Equal("Грузия", Active(storage).Name);

            var trips = bot.Handle(Msg("/trips"));
            var labels = trips.Buttons.SelectMany(r => r).Select(b => b.Label).ToList();
            Assert.Contains("✓ Грузия", labels);
            Assert.Contains("· Черногория", labels);

            bot.Handle(Msg("/use Черногория"));
            Assert.Equal("Черногория", Active(storage).Name);
        }

        [Fact]
        public void SwitchingWorksByAPartOfTheNameToo()
        {
            var (bot, storage) = TripWithThree();
            bot.Handle(Msg("/newtrip Грузия"));

            bot.Handle(Msg("/use черног"));
            Assert.Equal("Черногория", Active(storage).Name);
        }

        [Fact]
        public void AskingForATripThatIsNotThereSaysSo()
        {
            var (bot, _) = TripWithThree();
            Assert.Contains("Не нашёл", bot.Handle(Msg("/use Камчатка")).Text);
        }

        [Fact]
        public void OnlyOneTripIsEverActive()
        {
            var (bot, storage) = TripWithThree();
            bot.Handle(Msg("/newtrip Грузия"));
            bot.Handle(Msg("/use Черногория"));

            var all = storage.GetTours(t => true, false, 0, int.MaxValue, out _).ToList();
            Assert.Single(all, TgMeta.IsActive);
        }
    }
}
