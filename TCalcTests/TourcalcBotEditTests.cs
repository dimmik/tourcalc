using System.Linq;
using TCalc.Domain;
using TCalc.Storage;
using TCalcStorage.Storage;
using TCBlazor.Server.Telegram;
using Xunit;

namespace TCalcTests
{
    /// <summary>Renaming, re-weighting, re-parenting and removing people.</summary>
    public class TourcalcBotEditTests
    {
        private const long Chat = -1001234567;

        private static TgUpdate Msg(string text, long user = 1, string name = "Дима")
            => new TgUpdate { ChatId = Chat, UserId = user, DisplayName = name, UserName = name, Text = text };

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
            bot.Handle(Msg("/newtrip Черногория"));   // Дима
            bot.Handle(Msg("/add Маша"));
            return (bot, storage);
        }

        private static string IdOf(ITourStorage storage, string name)
            => Tour(storage).Persons.Single(p => p.Name == name).GUID;

        [Fact]
        public void APersonCanBeRenamed()
        {
            var (bot, storage) = Trip();
            var id = IdOf(storage, "Маша");

            var prompt = bot.Handle(Tap($"pren:{id}"));
            Assert.True(prompt.ForceReply);
            Assert.Contains("Маша", prompt.Text);

            bot.Handle(Answer(prompt.Text, "Мария"));
            Assert.Contains(Tour(storage).Persons, p => p.Name == "Мария");
            Assert.DoesNotContain(Tour(storage).Persons, p => p.Name == "Маша");
        }

        [Fact]
        public void RenamingKeepsTheSamePerson()
        {
            var (bot, storage) = Trip();
            var id = IdOf(storage, "Маша");
            bot.Handle(Answer(bot.Handle(Tap($"pren:{id}")).Text, "Мария"));

            Assert.Equal(id, IdOf(storage, "Мария"));   // not a delete-and-add
        }

        [Fact]
        public void AnEmptyNameIsRefused()
        {
            var (bot, storage) = Trip();
            var id = IdOf(storage, "Маша");
            var reply = bot.Handle(Answer(bot.Handle(Tap($"pren:{id}")).Text, "   "));

            Assert.Contains("пустым", reply.Text);
            Assert.Contains(Tour(storage).Persons, p => p.Name == "Маша");
        }

        [Fact]
        public void TheShareIsSetByButtonOrByTyping()
        {
            var (bot, storage) = Trip();
            var id = IdOf(storage, "Маша");

            bot.Handle(Tap($"pw:{id}:50"));
            Assert.Equal(50, Tour(storage).Persons.Single(p => p.Name == "Маша").Weight);

            var prompt = bot.Handle(Tap($"pw:{id}:x"));
            Assert.True(prompt.ForceReply);
            bot.Handle(Answer(prompt.Text, "37"));
            Assert.Equal(37, Tour(storage).Persons.Single(p => p.Name == "Маша").Weight);
        }

        [Fact]
        public void ANonsenseShareIsRefused()
        {
            var (bot, storage) = Trip();
            var id = IdOf(storage, "Маша");
            var reply = bot.Handle(Answer(bot.Handle(Tap($"pw:{id}:x")).Text, "много"));

            Assert.Contains("целое число", reply.Text);
            Assert.Equal(100, Tour(storage).Persons.Single(p => p.Name == "Маша").Weight);
        }

        [Fact]
        public void SomebodyCanBeMovedUnderAnotherPayerAndBackOut()
        {
            var (bot, storage) = Trip();
            var masha = IdOf(storage, "Маша");
            var dima = IdOf(storage, "Дима");

            bot.Handle(Tap($"par:{masha}:{dima}"));
            Assert.Equal(dima, Tour(storage).Persons.Single(p => p.GUID == masha).ParentId);

            bot.Handle(Tap($"par:{masha}:-"));
            Assert.True(string.IsNullOrEmpty(Tour(storage).Persons.Single(p => p.GUID == masha).ParentId));
        }

        [Fact]
        public void NobodyPaysForThemselvesAndNoCyclesAreAllowed()
        {
            var (bot, storage) = Trip();
            var masha = IdOf(storage, "Маша");
            var dima = IdOf(storage, "Дима");
            bot.Handle(Tap($"par:{masha}:{dima}"));      // Маша is under Дима

            // now try to put Дима under Маша - that would be a loop the settle-up never leaves
            var reply = bot.Handle(Tap($"par:{dima}:{masha}"));
            Assert.Contains("кольцо", reply.CallbackToast);
            Assert.True(string.IsNullOrEmpty(Tour(storage).Persons.Single(p => p.GUID == dima).ParentId));

            Assert.Contains("кольцо", bot.Handle(Tap($"par:{dima}:{dima}")).CallbackToast);
        }

        [Fact]
        public void ThePickerOffersOnlyPeopleWhoCouldBeAParent()
        {
            var (bot, storage) = Trip();
            var masha = IdOf(storage, "Маша");
            var picker = bot.Handle(Tap($"par:{masha}"));
            var labels = picker.Buttons.SelectMany(r => r).Select(b => b.Label).ToList();

            Assert.Contains("Дима", labels);
            Assert.DoesNotContain("Маша", labels);      // not herself
        }

        [Fact]
        public void DeletionAsksFirstAndSaysWhatElseGoes()
        {
            var (bot, storage) = Trip();
            bot.Handle(Msg("/spend 500 кофе"));          // paid by Дима
            var dima = IdOf(storage, "Дима");

            var confirm = bot.Handle(Tap($"pdel:{dima}"));
            Assert.Contains("Удалить Дима?", confirm.Text);
            Assert.Contains("траты", confirm.Text);      // warned before, not after
            Assert.Contains(Tour(storage).Persons, p => p.GUID == dima);   // nothing gone yet

            bot.Handle(Tap($"pdely:{dima}"));
            Assert.DoesNotContain(Tour(storage).Persons, p => p.GUID == dima);
            Assert.DoesNotContain(Tour(storage).Spendings, sp => !sp.Planned);   // his spending went too
        }

        [Fact]
        public void DeletingSomebodyWhoCarriesOthersWarnsAboutThem()
        {
            var (bot, storage) = Trip();
            bot.Handle(Msg("/covers Олег 35"));
            var dima = IdOf(storage, "Дима");

            Assert.Contains("станут платить за себя", bot.Handle(Tap($"pdel:{dima}")).Text);

            bot.Handle(Tap($"pdely:{dima}"));
            var oleg = Tour(storage).Persons.Single(p => p.Name == "Олег");
            Assert.True(string.IsNullOrEmpty(oleg.ParentId));   // detached, not deleted
        }

        [Fact]
        public void EditingSomebodyWhoIsAlreadyGoneSaysSoInsteadOfThrowing()
        {
            var (bot, _) = Trip();
            Assert.Contains("уже нет", bot.Handle(Tap("p:nosuchid")).CallbackToast);
            Assert.Contains("уже нет", bot.Handle(Tap("pdel:nosuchid")).CallbackToast);
            Assert.Contains("уже нет", bot.Handle(Tap("pw:nosuchid")).CallbackToast);
        }

        [Fact]
        public void ThePromptCarriesTheIdItAsksAbout()
        {
            // the reply Telegram sends back has only the prompt's text, so the id rides in it
            Assert.Equal("k47ppfq", TourcalcBot.PromptId("Новое имя для «Маша»? Ответь.\n#k47ppfq"));
            Assert.Equal("", TourcalcBot.PromptId("что-то без метки"));
        }
    }
}
