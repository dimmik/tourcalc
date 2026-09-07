using System.Linq;
using TCalc.Domain;
using TCalc.Storage;
using TCalcStorage.Storage;
using TCBlazor.Server.Telegram;
using Xunit;

namespace TCalcTests
{
    /// <summary>
    /// The bot driven end to end against the real in-memory storage and the real
    /// TourStorageProcessor - no Telegram, no token, no network. That is the whole point
    /// of keeping the transport out of TourcalcBot.
    /// </summary>
    public class TourcalcBotTests
    {
        private const long Chat = -1001234567;

        private static (TourcalcBot bot, ITourStorage storage) NewBot(TelegramBotOptions options = null)
        {
            var storage = new InMemoryTourStorage();
            var bot = new TourcalcBot(storage, new TourStorageProcessor(),
                options ?? new TelegramBotOptions { PublicBaseUrl = "https://tc.example.org" });
            return (bot, storage);
        }

        private static TgUpdate Msg(string text, long user = 1, string name = "Дима")
            => new TgUpdate { ChatId = Chat, UserId = user, DisplayName = name, UserName = name, Text = text };

        private static TgUpdate Tap(string data, long user = 1, string name = "Дима")
            => new TgUpdate { ChatId = Chat, UserId = user, DisplayName = name, UserName = name, CallbackData = data, CallbackMessageId = 10 };

        private Tour OnlyTour(ITourStorage storage)
            => storage.GetTours(t => true, false, 0, int.MaxValue, out _).Single(t => TgMeta.IsActive(t));

        [Fact]
        public void WithoutATripEverythingPointsAtCreatingOne()
        {
            var (bot, _) = NewBot();
            Assert.Contains("/newtrip", bot.Handle(Msg("/who")).Text);
            Assert.Contains("/newtrip", bot.Handle(Msg("/link")).Text);
            Assert.Contains("/newtrip", bot.Handle(Tap("join")).Text);
        }

        [Fact]
        public void NewTripCreatesATourBoundToTheChatWithItsAuthorInIt()
        {
            var (bot, storage) = NewBot();
            var reply = bot.Handle(Msg("/newtrip Черногория"));

            var tour = OnlyTour(storage);
            Assert.Equal("Черногория", tour.Name);
            Assert.Equal(Chat, TgMeta.ChatId(tour));
            Assert.True(TgMeta.IsActive(tour));
            Assert.False(string.IsNullOrWhiteSpace(tour.AccessCodeMD5));

            var person = Assert.Single(tour.Persons);
            Assert.Equal("Дима", person.Name);
            Assert.Equal(1, TgMeta.UserId(person));
            Assert.Contains("Черногория", reply.Text);
        }

        [Fact]
        public void NewTripNeedsAName()
        {
            var (bot, storage) = NewBot();
            bot.Handle(Msg("/newtrip"));
            Assert.Empty(storage.GetTours(t => true, false, 0, int.MaxValue, out _));
        }

        [Fact]
        public void TappingJoinAddsYouOnceAndEditsTheCardInPlace()
        {
            var (bot, storage) = NewBot();
            bot.Handle(Msg("/newtrip Черногория"));

            var joined = bot.Handle(Tap("join", user: 2, name: "Маша"));
            Assert.Equal(10, joined.EditMessageId);          // the card is edited, not re-posted
            Assert.Contains("Маша", joined.Text);
            Assert.Equal(2, OnlyTour(storage).Persons.Count);

            var again = bot.Handle(Tap("join", user: 2, name: "Маша"));
            Assert.Equal("Ты уже в списке", again.CallbackToast);
            Assert.Equal(2, OnlyTour(storage).Persons.Count);
        }

        [Fact]
        public void DependentsAreAddedAgainstThePersonWhoCarriesThem()
        {
            var (bot, storage) = NewBot();
            bot.Handle(Msg("/newtrip Черногория"));
            var reply = bot.Handle(Msg("/covers Олег 35"));

            var tour = OnlyTour(storage);
            var oleg = tour.Persons.Single(p => p.Name == "Олег");
            var dima = tour.Persons.Single(p => p.Name == "Дима");

            Assert.Equal(35, oleg.Weight);
            Assert.Equal(dima.GUID, oleg.ParentId);
            Assert.False(TgMeta.IsTelegramPerson(oleg));     // no Telegram account of his own
            Assert.Contains("Олег ×35", reply.Text);         // shown under the person carrying him
        }

        [Fact]
        public void TheCoversPromptCollectsTheNameAsAReply()
        {
            var (bot, storage) = NewBot();
            bot.Handle(Msg("/newtrip Черногория"));

            var prompt = bot.Handle(Tap("covers"));
            Assert.True(prompt.ForceReply);

            // the reply carries the prompt's text, which is how it is recognised without
            // keeping conversation state anywhere
            var reply = new TgUpdate
            {
                ChatId = Chat, UserId = 1, DisplayName = "Дима",
                Text = "Олег 35", ReplyToBotText = prompt.Text,
            };
            bot.Handle(reply);
            Assert.Contains(OnlyTour(storage).Persons, p => p.Name == "Олег" && p.Weight == 35);
        }

        [Fact]
        public void YouCannotCarrySomebodyBeforeJoiningYourself()
        {
            var (bot, storage) = NewBot();
            bot.Handle(Msg("/newtrip Черногория"));
            var reply = bot.Handle(Msg("/covers Олег 35", user: 99, name: "Чужой"));

            Assert.Contains("Я еду", reply.Text);
            Assert.DoesNotContain(OnlyTour(storage).Persons, p => p.Name == "Олег");
        }

        [Fact]
        public void EveryTripOfAChatSharesOneAccessCode()
        {
            // the reason: a link from this chat should open the chat's other trips too
            var (bot, storage) = NewBot();
            bot.Handle(Msg("/newtrip Черногория"));
            var first = OnlyTour(storage).AccessCodeMD5;

            bot.Handle(Msg("/newtrip Грузия"));
            var all = storage.GetTours(t => true, false, 0, int.MaxValue, out _).ToList();

            Assert.Equal(2, all.Count);
            Assert.All(all, t => Assert.Equal(first, t.AccessCodeMD5));
            Assert.Single(all, TgMeta.IsActive);                       // only one active
            Assert.Equal("Грузия", all.Single(TgMeta.IsActive).Name);        // the newest one
        }

        [Fact]
        public void DifferentChatsGetDifferentCodes()
        {
            var (bot, storage) = NewBot();
            bot.Handle(Msg("/newtrip Черногория"));
            bot.Handle(new TgUpdate { ChatId = 999, UserId = 1, DisplayName = "Дима", Text = "/newtrip Другая" });

            var codes = storage.GetTours(t => true, false, 0, int.MaxValue, out _)
                .Select(t => t.AccessCodeMD5).Distinct().ToList();
            Assert.Equal(2, codes.Count);
        }

        [Fact]
        public void LinkComesAsAButtonThatSignsTheReaderIn()
        {
            var (bot, storage) = NewBot();
            bot.Handle(Msg("/newtrip Черногория"));
            var tour = OnlyTour(storage);

            // a button rather than the address in the text: Telegram only linkifies hosts
            // it recognises, and a development server on localhost is not one of them
            var url = $"https://tc.example.org/goto/{tour.AccessCodeMD5}/{tour.Id}";
            var reply = bot.Handle(Msg("/link"));

            var button = reply.Buttons.SelectMany(r => r).Single();
            Assert.True(button.IsLink);
            Assert.Equal(url, button.Url);
            Assert.Null(button.Data);

            // and the address itself, which is what you copy or forward
            Assert.Contains(url, reply.Text);
        }

        [Fact]
        public void OnADevelopmentServerTheAddressGoesInTheTextInstead()
        {
            var storage = new InMemoryTourStorage();
            var bot = new TourcalcBot(storage, new TourStorageProcessor(),
                new TelegramBotOptions { PublicBaseUrl = "http://localhost:5399" });
            bot.Handle(Msg("/newtrip Черногория"));

            var reply = bot.Handle(Msg("/link"));
            Assert.Empty(reply.Buttons);                       // no button Telegram would refuse
            Assert.Contains("http://localhost:5399/goto/", reply.Text);
        }

        [Fact]
        public void TheTripCardOpensTheTourWithoutAnExtraMessage()
        {
            var (bot, storage) = NewBot();
            var card = bot.Handle(Msg("/newtrip Черногория"));
            var tour = OnlyTour(storage);

            var open = card.Buttons.SelectMany(r => r).Single(b => b.IsLink);
            Assert.Equal($"https://tc.example.org/goto/{tour.AccessCodeMD5}/{tour.Id}", open.Url);
        }

        [Fact]
        public void AChatOutsideTheAllowListIsIgnoredEntirely()
        {
            var (bot, storage) = NewBot(new TelegramBotOptions { AllowedChats = new long[] { 42 } });
            Assert.Null(bot.Handle(Msg("/newtrip Черногория")));
            Assert.Empty(storage.GetTours(t => true, false, 0, int.MaxValue, out _));
        }

        [Fact]
        public void ChatterIsIgnored()
        {
            var (bot, _) = NewBot();
            Assert.Null(bot.Handle(Msg("привет всем")));
            Assert.Null(bot.Handle(Msg("/somethingelse")));
        }
    }
}
