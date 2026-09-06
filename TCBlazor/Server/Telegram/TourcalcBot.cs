using System;
using System.Collections.Generic;
using System.Linq;
using System.Linq.Expressions;
using TCalc.Domain;
using TCalc.Storage;
using TCalcCore.Helpers;

namespace TCBlazor.Server.Telegram
{
    /// <summary>
    /// Everything the bot decides, with nothing that knows about Telegram, HTTP or ASP.NET.
    /// It takes a <see cref="TgUpdate"/> and returns a <see cref="TgReply"/>; the transport
    /// turns those into whatever Telegram wants. That boundary is what makes the bot
    /// testable without a token.
    ///
    /// The tours it writes are the same tours the web app reads: the same storage, the same
    /// TourStorageProcessor, the same calculator. The bot is another client, not a second
    /// implementation of the maths.
    /// </summary>
    public class TourcalcBot
    {
        /// <summary>Prefix of the prompt that asks for a dependent's name, recognised on the reply.</summary>
        internal const string CoversPrompt = "Кого ты везёшь?";

        private readonly ITourStorage storage;
        private readonly ITourStorageProcessor processor;
        private readonly TelegramBotOptions options;

        public TourcalcBot(ITourStorage storage, ITourStorageProcessor processor, TelegramBotOptions options)
        {
            this.storage = storage;
            this.processor = processor;
            this.options = options;
        }

        public TgReply Handle(TgUpdate update)
        {
            if (update == null) return null;
            if (!options.ChatAllowed(update.ChatId)) return null;

            if (!string.IsNullOrEmpty(update.CallbackData)) return OnButton(update);

            // a reply to the bot's own prompt is how free text arrives without turning
            // the bot's privacy mode off
            if (!string.IsNullOrWhiteSpace(update.ReplyToBotText)
                && update.ReplyToBotText.StartsWith(CoversPrompt, StringComparison.Ordinal))
            {
                return AddDependent(update, update.Text);
            }

            var command = TgCommand.Parse(update.Text);
            if (command == null) return null;

            if (command.Is("start") || command.Is("help")) return Help();
            if (command.Is("newtrip")) return NewTrip(update, command.Args);
            if (command.Is("who")) return WithTrip(update, TripCard);
            if (command.Is("link")) return WithTrip(update, (u, t) => Link(t));
            if (command.Is("covers")) return AddDependent(update, command.Args);

            return null;
        }

        // ------------------------------------------------------------------ commands --

        private TgReply Help() => TgReply.Say(
            "Учёт общих трат в этом чате.\n\n" +
            "/newtrip <название> — завести поездку\n" +
            "/who — кто едет\n" +
            "/link — открыть в браузере\n" +
            "/covers <Имя> [доля] — записать того, кого ты везёшь\n\n" +
            "Доля по умолчанию 100. Ребёнку обычно ставят меньше.");

        private TgReply NewTrip(TgUpdate update, string name)
        {
            name = (name ?? "").Trim();
            if (name.Length == 0) return TgReply.Say("Как назовём поездку? Например: /newtrip Черногония");

            // every trip of a chat shares one access code, so a link from this chat opens
            // all of them. The code itself is random and never stored in the clear - the
            // tour keeps only its MD5, which is all a /goto link needs.
            var codeMd5 = ExistingChatCodeMd5(update.ChatId) ?? NewRandomCodeMd5();

            foreach (var other in ChatTours(update.ChatId).Where(TgMeta.IsActive))
            {
                TgMeta.SetActive(other, false);
                storage.StoreTour(other);
            }

            var tour = new Tour
            {
                GUID = IdHelper.NewId(),
                Name = name,
                AccessCodeMD5 = codeMd5,
            };
            TgMeta.BindChat(tour, update.ChatId, active: true);
            storage.AddTour(tour);

            // whoever starts the trip is in it
            Join(tour, update);
            storage.StoreTour(tour);

            return TripCard(update, tour);
        }

        private TgReply AddDependent(TgUpdate update, string args)
        {
            var tour = ActiveTour(update.ChatId);
            if (tour == null) return NoTrip();

            var payer = FindPerson(tour, update.UserId);
            if (payer == null)
            {
                return TgReply.Say("Сначала нажми «Я еду» — тогда будет понятно, за кого ты платишь.");
            }

            if (!TryParseDependent(args, out var depName, out var weight))
            {
                return TgReply.Say("Не разобрал. Напиши имя и долю, например: Олег 35");
            }

            var dependent = new Person
            {
                GUID = IdHelper.NewId(),
                Name = depName,
                Weight = weight,
                ParentId = payer.GUID,
            };
            processor.AddPerson(tour, dependent);
            Save(tour);

            return TripCard(update, tour);
        }

        /// <summary>
        /// "Олег 35" - name, then an optional share. The share is the last word and only
        /// when it is a number, so "Дядя Вова" stays a name and gets the default of 100.
        /// </summary>
        internal static bool TryParseDependent(string args, out string name, out int weight)
        {
            name = "";
            weight = 100;
            var words = (args ?? "").Split(new[] { ' ', '\t' }, StringSplitOptions.RemoveEmptyEntries);
            if (words.Length == 0) return false;

            var last = words[words.Length - 1];
            if (words.Length > 1 && int.TryParse(last, out var w) && w >= 0)
            {
                weight = w;
                name = string.Join(" ", words.Take(words.Length - 1));
            }
            else
            {
                name = string.Join(" ", words);
            }
            return name.Length > 0;
        }

        // ------------------------------------------------------------------- buttons --

        private TgReply OnButton(TgUpdate update)
        {
            var tour = ActiveTour(update.ChatId);
            if (tour == null) return NoTrip();

            switch (update.CallbackData)
            {
                case "join":
                    if (FindPerson(tour, update.UserId) != null)
                        return TgReply.Toast("Ты уже в списке");
                    Join(tour, update);
                    Save(tour);
                    var card = TripCard(update, tour);
                    card.EditMessageId = update.CallbackMessageId;
                    return card;

                case "covers":
                    return new TgReply
                    {
                        Text = CoversPrompt + " Имя и доля через пробел.\nНапример: Олег 35 — полная доля 100.",
                        ForceReply = true,
                    };

                case "link":
                    return Link(tour);
            }
            return null;
        }

        // -------------------------------------------------------------------- pieces --

        private TgReply TripCard(TgUpdate update, Tour tour)
        {
            var lines = new List<string> { $"🧳 «{tour.Name}»" };
            var people = tour.Persons ?? new List<Person>();
            if (!people.Any())
            {
                lines.Add("Пока никого. Нажми «Я еду».");
            }
            else
            {
                lines.Add("Едут: " + string.Join(", ", people
                    .Where(p => string.IsNullOrEmpty(p.ParentId))
                    .OrderBy(p => p.Name)
                    .Select(p => Describe(people, p))));
            }

            return TgReply.Say(string.Join("\n", lines))
                .Row(new TgButton("Я еду", "join"), new TgButton("Я везу ещё кого-то", "covers"))
                .Row(new TgButton("Ссылка", "link"));
        }

        private static string Describe(IEnumerable<Person> all, Person person)
        {
            var kids = all.Where(k => k.ParentId == person.GUID).OrderBy(k => k.Name).ToList();
            if (!kids.Any()) return person.Name;
            return $"{person.Name} (+ {string.Join(", ", kids.Select(k => $"{k.Name} ×{k.Weight}"))})";
        }

        private TgReply Link(Tour tour)
            => TgReply.Say($"{options.PublicBaseUrl.TrimEnd('/')}/goto/{tour.AccessCodeMD5}/{tour.Id}");

        private static TgReply NoTrip()
            => TgReply.Say("В этом чате пока нет поездки. Заведи: /newtrip <название>");

        private void Join(Tour tour, TgUpdate update)
        {
            var person = new Person
            {
                GUID = IdHelper.NewId(),
                Name = string.IsNullOrWhiteSpace(update.DisplayName) ? update.UserName : update.DisplayName,
                Weight = 100,
            };
            TgMeta.BindUser(person, update.UserId, update.UserName);
            processor.AddPerson(tour, person);
        }

        private static Person FindPerson(Tour tour, long userId)
            => (tour.Persons ?? new List<Person>()).FirstOrDefault(p => TgMeta.UserId(p) == userId);

        private void Save(Tour tour)
        {
            // the app guards concurrent edits by comparing this; a change the bot makes
            // that left it alone would be overwritten by a browser tab holding an older copy
            tour.StateGUID = IdHelper.NewStateGuid();
            storage.StoreTour(tour);
        }

        private TgReply WithTrip(TgUpdate update, Func<TgUpdate, Tour, TgReply> f)
        {
            var tour = ActiveTour(update.ChatId);
            return tour == null ? NoTrip() : f(update, tour);
        }

        private IEnumerable<Tour> ChatTours(long chatId)
        {
            Expression<Func<Tour, bool>> all = t => true;
            var tours = storage.GetTours(all, false, 0, int.MaxValue, out _) ?? Enumerable.Empty<Tour>();
            return tours.Where(t => TgMeta.ChatId(t) == chatId).ToList();
        }

        private Tour ActiveTour(long chatId)
            => ChatTours(chatId).FirstOrDefault(TgMeta.IsActive);

        private string ExistingChatCodeMd5(long chatId)
            => ChatTours(chatId).Select(t => t.AccessCodeMD5).FirstOrDefault(c => !string.IsNullOrWhiteSpace(c));

        private static string NewRandomCodeMd5()
        {
            var bytes = new byte[24];
            using (var rng = System.Security.Cryptography.RandomNumberGenerator.Create()) rng.GetBytes(bytes);
            return Company.TCBlazor.Auth.AuthHelper.CreateMD5(Convert.ToBase64String(bytes));
        }
    }
}
