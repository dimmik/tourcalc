using System;
using System.Collections.Generic;
using System.Linq;
using System.Linq.Expressions;
using TCalc.Domain;
using TCalc.Logic;
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

        /// <summary>
        /// What a spending recorded from the chat is filed under. It must not be empty:
        /// the app treats a spending with no category as money moved to settle a debt
        /// rather than money spent, and it would go missing from every total.
        /// </summary>
        internal const string DefaultCategory = "прочее";

        private const int DefaultMinimumMeaningfulDebt = 49;

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
            if (command.Is("add")) return AddPerson(update, command.Args);
            if (command.Is("spend")) return Spend(update, command.Args);
            if (command.Is("balance")) return WithTrip(update, (u, t) => Balance(t));
            if (command.Is("settle")) return WithTrip(update, (u, t) => Settle(t));

            return null;
        }

        // ------------------------------------------------------------------ commands --

        private TgReply Help() => TgReply.Say(
            "Учёт общих трат в этом чате.\n\n" +
            "/newtrip <название> — завести поездку\n" +
            "/who — кто едет\n" +
            "/link — открыть в браузере\n" +
            "/covers <Имя> [доля] — записать того, кого ты везёшь\n" +
            "/add <Имя> [доля] — добавить человека без телеграма\n" +
            "/spend <сумма> <на что> — записать трату\n" +
            "/balance — кто сколько должен\n" +
            "/settle — кто кому платит\n\n" +
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

        /// <summary>
        /// Somebody who is on the trip but not in this chat - no Telegram account of their
        /// own, and unlike /covers they settle for themselves rather than through anybody.
        /// </summary>
        private TgReply AddPerson(TgUpdate update, string args)
        {
            var tour = ActiveTour(update.ChatId);
            if (tour == null) return NoTrip();

            if (!TryParseDependent(args, out var name, out var weight))
            {
                return TgReply.Say("Кого добавить? Например: /add Маша 50");
            }

            processor.AddPerson(tour, new Person
            {
                GUID = IdHelper.NewId(),
                Name = name,
                Weight = weight,
            });
            Save(tour);
            return TripCard(update, tour);
        }

        private TgReply Spend(TgUpdate update, string args)
        {
            var tour = ActiveTour(update.ChatId);
            if (tour == null) return NoTrip();

            var payer = FindPerson(tour, update.UserId);
            if (payer == null) return TgReply.Say("Сначала нажми «Я еду».");

            if (!TryParseSpending(args, out var amount, out var what))
            {
                return TgReply.Say("Не разобрал. Например: /spend 1200 такси из аэропорта");
            }

            var spending = new Spending
            {
                GUID = IdHelper.NewId(),
                AmountInCents = amount,
                Description = what,
                FromGuid = payer.GUID,
                ToAll = true,
                ToGuid = new List<string>(),
                // A spending with no category is not counted as spending at all - the app
                // reads that as money handed over to settle a debt. So one is always set.
                Type = DefaultCategory,
                Currency = tour.Currency,
                SpendingDate = DateTime.Now,
            };
            processor.AddSpending(tour, spending);
            Save(tour);

            return SpendingCard(tour, spending.GUID);
        }

        /// <summary>"1200 такси из аэропорта" - the amount first, then what it was for.</summary>
        internal static bool TryParseSpending(string args, out long amount, out string what)
        {
            amount = 0;
            what = "";
            var words = (args ?? "").Split(new[] { ' ', '\t' }, StringSplitOptions.RemoveEmptyEntries);
            if (words.Length < 2) return false;
            if (!long.TryParse(words[0], out amount) || amount <= 0) return false;
            what = string.Join(" ", words.Skip(1));
            return what.Length > 0;
        }

        private TgReply SpendingCard(Tour tour, string spendingId)
        {
            var spending = tour.Spendings.FirstOrDefault(sp => sp.GUID == spendingId);
            if (spending == null) return TgReply.Say("Трата не найдена — возможно, её уже удалили.");

            var payer = tour.Persons.FirstOrDefault(p => p.GUID == spending.FromGuid);
            return TgReply.Say(
                    $"💸 {Money(spending.AmountInCents)} — {spending.Description}\n" +
                    $"платил {payer?.Name ?? "?"} · на всех · {spending.Type}")
                .Row(new TgButton("платил не я", $"from:{spendingId}"),
                     new TgButton("🗑", $"del:{spendingId}"));
        }

        private TgReply Balance(Tour tour)
        {
            var calculated = Calculated(tour);
            var min = MinMeaningful(calculated);
            var people = calculated.Persons ?? new List<Person>();

            var counted = calculated.Spendings.Count(sp => !sp.Planned);
            var spent = calculated.Spendings
                .Where(sp => !sp.Planned && !string.IsNullOrWhiteSpace(sp.Type) && (!sp.IsDryRun || sp.IncludeDryRunInCalc))
                .Sum(sp => sp.AmountInCents);

            var lines = new List<string>
            {
                $"🧳 «{calculated.Name}»",
                $"Потрачено {Money(spent)} · {people.Count} чел. · {counted} трат",
                "",
            };

            foreach (var person in people.Where(p => string.IsNullOrEmpty(p.ParentId)).OrderBy(p => p.Name))
            {
                lines.Add(Settlement(calculated, person, min));
                foreach (var kid in people.Where(k => k.ParentId == person.GUID).OrderBy(k => k.Name))
                {
                    lines.Add($"   └ {kid.Name} — через {person.Name}");
                }
            }

            return TgReply.Say(string.Join("\n", lines))
                .Row(new TgButton("Кто кому платит", "settle"), new TgButton("Открыть", "link"));
        }

        private static string Settlement(Tour calculated, Person person, long min)
        {
            var amount = calculated.AmountAPersonWillPay(person);
            if (Math.Abs(amount) <= min) return $"{person.Name} — в расчёте";
            // infinitives on purpose: a name says nothing about gender, and "должен"
            // against half the people in a Russian chat reads as a mistake
            return amount > 0
                ? $"{person.Name} — отдать {Money(amount)}"
                : $"{person.Name} — получить {Money(-amount)}";
        }

        private TgReply Settle(Tour tour)
        {
            var calculated = Calculated(tour);
            var transfers = Transfers(calculated);
            if (!transfers.Any()) return TgReply.Say("Все в расчёте — платить некому.");

            var reply = TgReply.Say("Кто кому платит (пока никто ничего не отдал):");
            foreach (var transfer in transfers)
            {
                var from = PersonName(calculated, transfer.FromGuid);
                var to = PersonName(calculated, transfer.ToGuid.FirstOrDefault());
                reply.Text += $"\n{from} → {to}  {Money(transfer.AmountInCents)}";
                reply.Row(new TgButton($"✓ {from} → {to} {Money(transfer.AmountInCents)}", $"paid:{transfer.GUID}"));
            }
            return reply;
        }

        /// <summary>
        /// Records that one of the suggested payments happened, the way the app's "Mark
        /// paid" does: a copy with no category, which moves both balances without counting
        /// as money spent on the trip.
        /// </summary>
        private TgReply MarkPaid(Tour tour, string transferId)
        {
            var calculated = Calculated(tour);
            var planned = calculated.Spendings.FirstOrDefault(sp => sp.Planned && sp.GUID == transferId);
            if (planned == null) return TgReply.Toast("Этот платёж уже не актуален");

            var copy = planned.SafeClone<Spending>();
            copy.Type = "";
            copy.Color = copy.Description.StartsWith("X") ? "lightgreen" : "lightgray";
            processor.AddSpending(tour, copy);
            Save(tour);

            return Settle(tour);
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

                case "settle":
                    return Settle(tour);
            }

            // the ones that carry an id: "verb:id" and, for the payer picker, "from:sp:person"
            var parts = update.CallbackData.Split(':');
            if (parts.Length >= 2)
            {
                switch (parts[0])
                {
                    case "from" when parts.Length == 2:
                        return PayerPicker(tour, parts[1], update.CallbackMessageId);

                    case "from" when parts.Length == 3:
                        return SetPayer(tour, parts[1], parts[2], update.CallbackMessageId);

                    case "del":
                        return DeleteSpending(tour, parts[1], update.CallbackMessageId);

                    case "paid":
                        return MarkPaid(tour, parts[1]);
                }
            }
            return null;
        }

        private TgReply PayerPicker(Tour tour, string spendingId, int? messageId)
        {
            var reply = TgReply.Say("Кто платил?");
            reply.EditMessageId = messageId;
            // one per row: names are long enough that two across get truncated on a phone
            foreach (var person in (tour.Persons ?? new List<Person>()).OrderBy(p => p.Name))
            {
                reply.Row(new TgButton(person.Name, $"from:{spendingId}:{person.GUID}"));
            }
            return reply;
        }

        private TgReply SetPayer(Tour tour, string spendingId, string personId, int? messageId)
        {
            var spending = tour.Spendings.FirstOrDefault(sp => sp.GUID == spendingId);
            if (spending == null) return TgReply.Toast("Трата не найдена");
            if (!(tour.Persons ?? new List<Person>()).Any(p => p.GUID == personId)) return TgReply.Toast("Нет такого человека");

            spending.FromGuid = personId;
            // UpdateSpending drops the planned payments the change invalidates
            processor.UpdateSpending(tour, spending, spendingId);
            Save(tour);

            var card = SpendingCard(tour, spendingId);
            card.EditMessageId = messageId;
            return card;
        }

        private TgReply DeleteSpending(Tour tour, string spendingId, int? messageId)
        {
            var spending = tour.Spendings.FirstOrDefault(sp => sp.GUID == spendingId);
            if (spending == null) return TgReply.Toast("Уже удалена");

            processor.DeleteSpending(tour, spendingId);
            Save(tour);

            return new TgReply
            {
                Text = $"🗑 удалено: {spending.Description} — {Money(spending.AmountInCents)}",
                EditMessageId = messageId,
            };
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

        /// <summary>
        /// The tour with everyone's totals filled in and the settle-up payments worked out.
        /// The calculator clones what it is given, so this never disturbs the stored tour -
        /// which matters, because a calculated tour carries the suggested payments as if
        /// they were real spendings and must never be saved.
        /// </summary>
        private static Tour Calculated(Tour tour) => new TourCalculator(tour).SuggestFinalPayments();

        /// <summary>
        /// Anything smaller than this counts as settled. The app takes it from the reader's
        /// own settings, which live in their browser; the bot uses the shipped default.
        /// </summary>
        private static long MinMeaningful(Tour tour)
            => tour.GetAmountInCurrentCurrencyFromMinValued(DefaultMinimumMeaningfulDebt);

        private static List<Spending> Transfers(Tour calculated)
        {
            var min = MinMeaningful(calculated);
            return calculated.Spendings
                .Where(sp => sp.Planned && !sp.Description.StartsWith("Family"))
                .Where(sp => Math.Abs(sp.AmountInCents) > min)
                .OrderByDescending(sp => sp.AmountInCents)
                .ToList();
        }

        private static string PersonName(Tour tour, string guid)
            => (tour.Persons ?? new List<Person>()).FirstOrDefault(p => p.GUID == guid)?.Name ?? "?";

        /// <summary>Money written the way the rest of the app writes it.</summary>
        private static string Money(long amount)
            => amount.ToString("N0", TCalcCore.UI.GlobConsts.NumGroupSpaceSeparated);

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
