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

        internal const string RenamePrompt = "Новое имя";
        internal const string WeightPrompt = "Какая доля";
        internal const string CategoryPrompt = "Категория";
        internal const string AmountPrompt = "Новая сумма";
        internal const string DescriptionPrompt = "Новое описание";

        /// <summary>
        /// A prompt that asks about a particular person has to say which one, and the reply
        /// Telegram sends back carries only the prompt's own text. Rather than keep a map of
        /// who was asked what - which a restart would lose and a second person in the chat
        /// could collide with - the prompt ends with the id and the reply is read from it.
        /// </summary>
        private const string IdMarker = "\n#";

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
            if (!string.IsNullOrWhiteSpace(update.ReplyToBotText))
            {
                var prompt = update.ReplyToBotText;
                if (prompt.StartsWith(CoversPrompt, StringComparison.Ordinal))
                {
                    return AddDependent(update, update.Text);
                }
                if (prompt.StartsWith(RenamePrompt, StringComparison.Ordinal))
                {
                    return Rename(update, PromptId(prompt), update.Text);
                }
                if (prompt.StartsWith(WeightPrompt, StringComparison.Ordinal))
                {
                    return SetWeight(update, PromptId(prompt), update.Text);
                }
                if (prompt.StartsWith(CategoryPrompt, StringComparison.Ordinal))
                {
                    var tour = ActiveTour(update.ChatId);
                    return tour == null ? NoTrip() : SetCategory(tour, PromptId(prompt), update.Text, null);
                }
                if (prompt.StartsWith(AmountPrompt, StringComparison.Ordinal))
                {
                    var tour = ActiveTour(update.ChatId);
                    return tour == null ? NoTrip() : SetAmount(tour, PromptId(prompt), update.Text);
                }
                if (prompt.StartsWith(DescriptionPrompt, StringComparison.Ordinal))
                {
                    var tour = ActiveTour(update.ChatId);
                    return tour == null ? NoTrip() : SetDescription(tour, PromptId(prompt), update.Text);
                }
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
            if (command.Is("spendings")) return WithTrip(update, (u, t) => SpendingList(t, null));
            if (command.Is("trips")) return Trips(update, null);
            if (command.Is("use")) return UseByName(update, command.Args);
            if (command.Is("balance")) return WithTrip(update, (u, t) => Balance(t, u.IsPrivate));
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
            "/spendings — список трат, чтобы поправить старую\n" +
            "/balance — кто сколько должен\n" +
            "/settle — кто кому платит\n" +
            "/trips — поездки чата, /use <название> — переключиться\n\n" +
            "Поправить имя, долю, кто за кого платит или удалить — " +
            "кнопка «Изменить» в /who.\n\n" +
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

        private TgReply SpendingCard(Tour tour, string spendingId, int? messageId = null)
        {
            var spending = tour.Spendings.FirstOrDefault(sp => sp.GUID == spendingId);
            if (spending == null) return TgReply.Say("Трата не найдена — возможно, её уже удалили.");

            var payer = tour.Persons.FirstOrDefault(p => p.GUID == spending.FromGuid);
            var card = TgReply.Say(
                    $"💸 {Money(spending.AmountInCents)} — {spending.Description}\n" +
                    $"платил {payer?.Name ?? "?"} · {ForWhom(tour, spending)} · {spending.Type}")
                .Row(new TgButton("сумма", $"amt:{spendingId}"),
                     new TgButton("описание", $"desc:{spendingId}"))
                .Row(new TgButton("платил не я", $"from:{spendingId}"),
                     new TgButton("не на всех", $"to:{spendingId}"))
                .Row(new TgButton("категория", $"cat:{spendingId}"),
                     new TgButton("🗑", $"del:{spendingId}"));
            card.EditMessageId = messageId;
            return card;
        }

        private TgReply Balance(Tour tour, bool isPrivate)
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

            var reply = TgReply.Say(string.Join("\n", lines));
            AddOpenButton(reply, calculated, isPrivate, new TgButton("Кто кому платит", "settle"));
            return reply;
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

        // ---------------------------------------------------- several trips a chat --

        private TgReply Trips(TgUpdate update, int? messageId)
        {
            var tours = ChatTours(update.ChatId).OrderByDescending(t => t.DateCreated).ToList();
            if (!tours.Any()) return NoTrip();

            var reply = TgReply.Say("Поездки этого чата. Текущая отмечена галочкой.");
            reply.EditMessageId = messageId;
            foreach (var tour in tours)
            {
                var mark = TgMeta.IsActive(tour) ? "✓" : "·";
                reply.Row(new TgButton($"{mark} {tour.Name}", $"use:{tour.Id}"));
            }
            return reply;
        }

        private TgReply UseByName(TgUpdate update, string name)
        {
            name = (name ?? "").Trim();
            if (name.Length == 0) return Trips(update, null);

            var tours = ChatTours(update.ChatId).ToList();
            var found = tours.FirstOrDefault(t => string.Equals(t.Name, name, StringComparison.OrdinalIgnoreCase))
                     ?? tours.FirstOrDefault(t => (t.Name ?? "").IndexOf(name, StringComparison.OrdinalIgnoreCase) >= 0);
            if (found == null) return TgReply.Say($"Не нашёл поездку «{name}». Список: /trips");

            return Activate(update, found.Id, null);
        }

        /// <summary>
        /// Makes one trip of this chat the current one. Only one is ever active, so every
        /// command that says "the trip" has exactly one answer.
        /// </summary>
        private TgReply Activate(TgUpdate update, string tourId, int? messageId)
        {
            var tours = ChatTours(update.ChatId).ToList();
            var wanted = tours.FirstOrDefault(t => t.Id == tourId);
            if (wanted == null) return TgReply.Toast("Такой поездки в этом чате нет");

            foreach (var tour in tours)
            {
                var shouldBeActive = tour.Id == tourId;
                if (TgMeta.IsActive(tour) == shouldBeActive) continue;
                TgMeta.SetActive(tour, shouldBeActive);
                storage.StoreTour(tour);
            }

            var card = TripCard(update, wanted);
            card.EditMessageId = messageId;
            return card;
        }

        // ------------------------------------------------------ the list of spendings --

        /// <summary>How many spendings the list shows. A Telegram keyboard has to stay tappable.</summary>
        private const int SpendingsShown = 15;

        /// <summary>
        /// Past spendings, each one a button that opens its card.
        ///
        /// Without this a spending could only ever be touched in the moment it was created:
        /// the card scrolled away with the conversation and nothing led back to it, so a
        /// mistyped amount meant opening the web app.
        /// </summary>
        private TgReply SpendingList(Tour tour, int? messageId)
        {
            var spendings = (tour.Spendings ?? new List<Spending>())
                .Where(sp => !sp.Planned)
                .OrderByDescending(sp => sp.SpendingDate)
                .ThenByDescending(sp => sp.DateCreated)
                .Take(SpendingsShown)
                .ToList();

            if (!spendings.Any()) return TgReply.Say("Трат пока нет. Первая: /spend 1200 такси");

            var total = (tour.Spendings ?? new List<Spending>()).Count(sp => !sp.Planned);
            var reply = TgReply.Say(total > spendings.Count
                ? $"Последние {spendings.Count} из {total}. Нажми, чтобы поправить."
                : "Траты. Нажми, чтобы поправить.");
            reply.EditMessageId = messageId;

            foreach (var spending in spendings)
            {
                var who = PersonName(tour, spending.FromGuid);
                var label = $"{spending.SpendingDate:dd.MM} · {Trim(spending.Description, 22)} · {Money(spending.AmountInCents)} · {Trim(who, 10)}";
                reply.Row(new TgButton(label, $"sp:{spending.GUID}"));
            }
            return reply;
        }

        /// <summary>Button text has to fit on a phone, so the long parts are cut.</summary>
        private static string Trim(string text, int max)
        {
            text = (text ?? "").Trim();
            return text.Length <= max ? text : text.Substring(0, Math.Max(1, max - 1)) + "…";
        }

        private TgReply SetAmount(Tour tour, string spendingId, string text)
        {
            var spending = tour.Spendings.FirstOrDefault(sp => sp.GUID == spendingId);
            if (spending == null) return TgReply.Say("Трата не найдена — возможно, её уже удалили.");

            if (!long.TryParse((text ?? "").Trim(), out var amount) || amount <= 0)
            {
                return TgReply.Say("Сумма — целое число больше нуля.");
            }

            spending.AmountInCents = amount;
            processor.UpdateSpending(tour, spending, spendingId);
            Save(tour);
            return SpendingCard(tour, spendingId);
        }

        private TgReply SetDescription(Tour tour, string spendingId, string text)
        {
            var spending = tour.Spendings.FirstOrDefault(sp => sp.GUID == spendingId);
            if (spending == null) return TgReply.Say("Трата не найдена — возможно, её уже удалили.");

            text = (text ?? "").Trim();
            if (text.Length == 0) return TgReply.Say("Описание не может быть пустым.");

            spending.Description = text;
            processor.UpdateSpending(tour, spending, spendingId);
            Save(tour);
            return SpendingCard(tour, spendingId);
        }

        // ------------------------------------------------- who a spending is for --

        private static string ForWhom(Tour tour, Spending spending)
        {
            if (spending.ToAll) return "на всех";
            var names = (spending.ToGuid ?? new List<string>())
                .Select(g => PersonName(tour, g)).OrderBy(n => n).ToList();
            if (names.Count == 0) return "ни на кого";
            return names.Count <= 3 ? "на " + string.Join(", ", names) : $"на {names.Count} человек";
        }

        private TgReply SharePicker(Tour tour, string spendingId, int? messageId)
        {
            var spending = tour.Spendings.FirstOrDefault(sp => sp.GUID == spendingId);
            if (spending == null) return TgReply.Toast("Трата не найдена");

            var reply = TgReply.Say($"На кого делим «{spending.Description}»?\nСейчас: {ForWhom(tour, spending)}");
            reply.EditMessageId = messageId;
            foreach (var person in (tour.Persons ?? new List<Person>()).OrderBy(p => p.Name))
            {
                var on = spending.ToAll || (spending.ToGuid ?? new List<string>()).Contains(person.GUID);
                reply.Row(new TgButton($"{(on ? "✓" : "·")} {person.Name}", $"to:{spendingId}:{person.GUID}"));
            }
            reply.Row(new TgButton("на всех", $"to:{spendingId}:*"), new TgButton("готово", $"sp:{spendingId}"));
            return reply;
        }

        private TgReply ToggleShare(Tour tour, string spendingId, string personId, int? messageId)
        {
            var spending = tour.Spendings.FirstOrDefault(sp => sp.GUID == spendingId);
            if (spending == null) return TgReply.Toast("Трата не найдена");

            if (personId == "*")
            {
                spending.ToAll = true;
                spending.ToGuid = new List<string>();
            }
            else
            {
                if (!(tour.Persons ?? new List<Person>()).Any(p => p.GUID == personId))
                {
                    return TgReply.Toast("Нет такого человека");
                }
                // leaving "everyone" starts from everyone, so unticking one person keeps
                // the rest rather than emptying the list
                var chosen = spending.ToAll
                    ? (tour.Persons ?? new List<Person>()).Select(p => p.GUID).ToList()
                    : new List<string>(spending.ToGuid ?? new List<string>());

                if (!chosen.Remove(personId)) chosen.Add(personId);

                if (chosen.Count == 0)
                {
                    // a spending for nobody is not a thing the calculator can split
                    spending.ToAll = true;
                    spending.ToGuid = new List<string>();
                }
                else
                {
                    spending.ToAll = false;
                    spending.ToGuid = chosen;
                }
            }

            processor.UpdateSpending(tour, spending, spendingId);
            Save(tour);
            return SharePicker(tour, spendingId, messageId);
        }

        // ------------------------------------------------------------- categories --

        internal static List<string> Categories(Tour tour)
            => (tour.Spendings ?? new List<Spending>())
                .Where(sp => !sp.Planned && !string.IsNullOrWhiteSpace(sp.Type))
                .Select(sp => sp.Type)
                .Distinct()
                .OrderBy(t => t)
                .ToList();

        private TgReply CategoryPicker(Tour tour, string spendingId, int? messageId)
        {
            var spending = tour.Spendings.FirstOrDefault(sp => sp.GUID == spendingId);
            if (spending == null) return TgReply.Toast("Трата не найдена");

            var reply = TgReply.Say($"Категория для «{spending.Description}»? Сейчас: {spending.Type}");
            reply.EditMessageId = messageId;
            var categories = Categories(tour);
            // by position, not by name: a category is free text and would not survive the
            // trip through callback data, which Telegram caps at 64 bytes
            for (var i = 0; i < categories.Count; i++)
            {
                reply.Row(new TgButton(categories[i], $"cat:{spendingId}:{i}"));
            }
            reply.Row(new TgButton("своя", $"cat:{spendingId}:x"), new TgButton("готово", $"sp:{spendingId}"));
            return reply;
        }

        private TgReply SetCategory(Tour tour, string spendingId, string category, int? messageId)
        {
            var spending = tour.Spendings.FirstOrDefault(sp => sp.GUID == spendingId);
            if (spending == null) return TgReply.Toast("Трата не найдена");

            category = (category ?? "").Trim();
            if (category.Length == 0) return TgReply.Say("Категория не может быть пустой.");

            spending.Type = category;
            processor.UpdateSpending(tour, spending, spendingId);
            Save(tour);
            return SpendingCard(tour, spendingId, messageId);
        }

        // --------------------------------------------------------- editing people --

        private TgReply PeoplePicker(Tour tour, int? messageId)
        {
            var reply = TgReply.Say("Кого поправить?");
            reply.EditMessageId = messageId;
            foreach (var person in (tour.Persons ?? new List<Person>()).OrderBy(p => p.Name))
            {
                reply.Row(new TgButton(person.Name, $"p:{person.GUID}"));
            }
            reply.Row(new TgButton("← назад", "who"));
            return reply;
        }

        private TgReply PersonCard(Tour tour, string personId, int? messageId)
        {
            var person = Find(tour, personId);
            if (person == null) return TgReply.Toast("Такого человека уже нет");

            var parent = string.IsNullOrEmpty(person.ParentId) ? null : Find(tour, person.ParentId);
            var lines = new List<string> { $"{person.Name} · доля {person.Weight}" };
            lines.Add(parent == null ? "платит за себя" : $"за него платит {parent.Name}");

            var kids = (tour.Persons ?? new List<Person>()).Where(k => k.ParentId == person.GUID).ToList();
            if (kids.Any()) lines.Add("везёт: " + string.Join(", ", kids.Select(k => k.Name)));

            var reply = TgReply.Say(string.Join("\n", lines));
            reply.EditMessageId = messageId;
            reply.Row(new TgButton("переименовать", $"pren:{personId}"), new TgButton("доля", $"pw:{personId}"));
            reply.Row(new TgButton("кто за него платит", $"par:{personId}"));
            reply.Row(new TgButton("удалить", $"pdel:{personId}"), new TgButton("← назад", "edit"));
            return reply;
        }

        private TgReply Rename(TgUpdate update, string personId, string name)
        {
            var tour = ActiveTour(update.ChatId);
            if (tour == null) return NoTrip();
            var person = Find(tour, personId);
            if (person == null) return TgReply.Say("Такого человека уже нет.");

            name = (name ?? "").Trim();
            if (name.Length == 0) return TgReply.Say("Имя не может быть пустым.");

            person.Name = name;
            processor.UpdatePerson(tour, person, personId);
            Save(tour);
            return TripCard(update, tour);
        }

        private TgReply SetWeight(TgUpdate update, string personId, string text)
        {
            var tour = ActiveTour(update.ChatId);
            if (tour == null) return NoTrip();
            if (!int.TryParse((text ?? "").Trim(), out var weight) || weight < 0)
            {
                return TgReply.Say("Доля — целое число от 0. Полная доля 100.");
            }
            return ApplyWeight(tour, personId, weight, null) ?? TripCard(update, tour);
        }

        private TgReply ApplyWeight(Tour tour, string personId, int weight, int? messageId)
        {
            var person = Find(tour, personId);
            if (person == null) return TgReply.Toast("Такого человека уже нет");

            person.Weight = weight;
            processor.UpdatePerson(tour, person, personId);
            Save(tour);
            return PersonCard(tour, personId, messageId);
        }

        private TgReply WeightPicker(Tour tour, string personId, int? messageId)
        {
            var person = Find(tour, personId);
            if (person == null) return TgReply.Toast("Такого человека уже нет");

            var reply = TgReply.Say($"Доля для {person.Name}? Сейчас {person.Weight}. Полная доля — 100.");
            reply.EditMessageId = messageId;
            reply.Row(new TgButton("100", $"pw:{personId}:100"), new TgButton("50", $"pw:{personId}:50"),
                      new TgButton("35", $"pw:{personId}:35"));
            reply.Row(new TgButton("25", $"pw:{personId}:25"), new TgButton("0", $"pw:{personId}:0"),
                      new TgButton("своё", $"pw:{personId}:x"));
            reply.Row(new TgButton("← назад", $"p:{personId}"));
            return reply;
        }

        private TgReply ParentPicker(Tour tour, string personId, int? messageId)
        {
            var person = Find(tour, personId);
            if (person == null) return TgReply.Toast("Такого человека уже нет");

            var reply = TgReply.Say($"Кто платит за {person.Name}?");
            reply.EditMessageId = messageId;
            reply.Row(new TgButton("никто — платит за себя", $"par:{personId}:-"));
            foreach (var other in (tour.Persons ?? new List<Person>())
                     .Where(p => CanBeParentOf(tour, p, person))
                     .OrderBy(p => p.Name))
            {
                reply.Row(new TgButton(other.Name, $"par:{personId}:{other.GUID}"));
            }
            reply.Row(new TgButton("← назад", $"p:{personId}"));
            return reply;
        }

        /// <summary>
        /// Nobody may pay for themselves, and nobody may be moved under one of their own
        /// dependents: either would make a cycle the settle-up walk never leaves.
        /// </summary>
        internal static bool CanBeParentOf(Tour tour, Person candidate, Person person)
        {
            if (candidate.GUID == person.GUID) return false;
            var all = tour.Persons ?? new List<Person>();
            var cursor = candidate;
            for (var guard = 0; cursor != null && guard < 50; guard++)
            {
                if (cursor.ParentId == person.GUID) return false;
                cursor = all.FirstOrDefault(p => p.GUID == cursor.ParentId);
            }
            return true;
        }

        private TgReply SetParent(Tour tour, string personId, string parentId, int? messageId)
        {
            var person = Find(tour, personId);
            if (person == null) return TgReply.Toast("Такого человека уже нет");

            if (parentId == "-")
            {
                person.ParentId = null;
            }
            else
            {
                var parent = Find(tour, parentId);
                if (parent == null || !CanBeParentOf(tour, parent, person))
                {
                    return TgReply.Toast("Так нельзя — получится кольцо");
                }
                person.ParentId = parentId;
            }

            processor.UpdatePerson(tour, person, personId);
            Save(tour);
            return PersonCard(tour, personId, messageId);
        }

        private TgReply ConfirmDelete(Tour tour, string personId, int? messageId)
        {
            var person = Find(tour, personId);
            if (person == null) return TgReply.Toast("Такого человека уже нет");

            var spendings = tour.Spendings.Count(sp => !sp.Planned && sp.FromGuid == personId);
            var kids = (tour.Persons ?? new List<Person>()).Count(k => k.ParentId == personId);

            var lines = new List<string> { $"Удалить {person.Name}?" };
            // deleting a person takes their spendings with them - worth saying before, not after
            if (spendings > 0) lines.Add($"Вместе с ним удалятся его траты: {spendings}.");
            if (kids > 0) lines.Add($"Те, кого он везёт ({kids}), станут платить за себя.");

            var reply = TgReply.Say(string.Join("\n", lines));
            reply.EditMessageId = messageId;
            reply.Row(new TgButton("да, удалить", $"pdely:{personId}"), new TgButton("отмена", $"p:{personId}"));
            return reply;
        }

        private TgReply DeletePerson(TgUpdate update, Tour tour, string personId, int? messageId)
        {
            var person = Find(tour, personId);
            if (person == null) return TgReply.Toast("Такого человека уже нет");

            processor.DeletePerson(tour, personId);
            Save(tour);

            var card = TripCard(update, tour);
            card.EditMessageId = messageId;
            return card;
        }

        private static Person Find(Tour tour, string personId)
            => (tour.Persons ?? new List<Person>()).FirstOrDefault(p => p.GUID == personId);

        /// <summary>The person id a prompt was about, taken off its last line.</summary>
        internal static string PromptId(string promptText)
        {
            var at = (promptText ?? "").LastIndexOf(IdMarker, StringComparison.Ordinal);
            return at < 0 ? "" : promptText.Substring(at + IdMarker.Length).Trim();
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

                case "who":
                    var back = TripCard(update, tour);
                    back.EditMessageId = update.CallbackMessageId;
                    return back;

                case "edit":
                    return PeoplePicker(tour, update.CallbackMessageId);

                case "trips":
                    return Trips(update, update.CallbackMessageId);

                case "spendings":
                    return SpendingList(tour, update.CallbackMessageId);
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

                    case "p":
                        return PersonCard(tour, parts[1], update.CallbackMessageId);

                    case "pren":
                        var who = Find(tour, parts[1]);
                        if (who == null) return TgReply.Toast("Такого человека уже нет");
                        return new TgReply
                        {
                            Text = $"{RenamePrompt} для «{who.Name}»? Ответь на это сообщение.{IdMarker}{parts[1]}",
                            ForceReply = true,
                        };

                    case "pw" when parts.Length == 2:
                        return WeightPicker(tour, parts[1], update.CallbackMessageId);

                    case "pw" when parts.Length == 3 && parts[2] == "x":
                        var forWhom = Find(tour, parts[1]);
                        if (forWhom == null) return TgReply.Toast("Такого человека уже нет");
                        return new TgReply
                        {
                            Text = $"{WeightPrompt} у «{forWhom.Name}»? Ответь числом.{IdMarker}{parts[1]}",
                            ForceReply = true,
                        };

                    case "pw" when parts.Length == 3 && int.TryParse(parts[2], out var weight):
                        return ApplyWeight(tour, parts[1], weight, update.CallbackMessageId);

                    case "par" when parts.Length == 2:
                        return ParentPicker(tour, parts[1], update.CallbackMessageId);

                    case "par" when parts.Length == 3:
                        return SetParent(tour, parts[1], parts[2], update.CallbackMessageId);

                    case "pdel":
                        return ConfirmDelete(tour, parts[1], update.CallbackMessageId);

                    case "pdely":
                        return DeletePerson(update, tour, parts[1], update.CallbackMessageId);

                    case "use":
                        return Activate(update, parts[1], update.CallbackMessageId);

                    case "sp":
                        return SpendingCard(tour, parts[1], update.CallbackMessageId);

                    case "amt":
                        var forAmount = tour.Spendings.FirstOrDefault(sp => sp.GUID == parts[1]);
                        if (forAmount == null) return TgReply.Toast("Трата не найдена");
                        return new TgReply
                        {
                            Text = $"{AmountPrompt} для «{forAmount.Description}»? Сейчас {Money(forAmount.AmountInCents)}. Ответь числом.{IdMarker}{parts[1]}",
                            ForceReply = true,
                        };

                    case "desc":
                        var forText = tour.Spendings.FirstOrDefault(sp => sp.GUID == parts[1]);
                        if (forText == null) return TgReply.Toast("Трата не найдена");
                        return new TgReply
                        {
                            Text = $"{DescriptionPrompt} вместо «{forText.Description}»? Ответь на это сообщение.{IdMarker}{parts[1]}",
                            ForceReply = true,
                        };

                    case "to" when parts.Length == 2:
                        return SharePicker(tour, parts[1], update.CallbackMessageId);

                    case "to" when parts.Length == 3:
                        return ToggleShare(tour, parts[1], parts[2], update.CallbackMessageId);

                    case "cat" when parts.Length == 2:
                        return CategoryPicker(tour, parts[1], update.CallbackMessageId);

                    case "cat" when parts.Length == 3 && parts[2] == "x":
                        return new TgReply
                        {
                            Text = $"{CategoryPrompt} для этой траты? Ответь одним словом.{IdMarker}{parts[1]}",
                            ForceReply = true,
                        };

                    case "cat" when parts.Length == 3 && int.TryParse(parts[2], out var index):
                        var categories = Categories(tour);
                        if (index < 0 || index >= categories.Count) return TgReply.Toast("Категория пропала");
                        return SetCategory(tour, parts[1], categories[index], update.CallbackMessageId);
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

            var card = TgReply.Say(string.Join("\n", lines))
                .Row(new TgButton("Я еду", "join"), new TgButton("Я везу ещё кого-то", "covers"));
            AddOpenButton(card, tour, update.IsPrivate, new TgButton("Изменить", "edit"));
            return card;
        }

        private static string Describe(IEnumerable<Person> all, Person person)
        {
            var kids = all.Where(k => k.ParentId == person.GUID).OrderBy(k => k.Name).ToList();
            if (!kids.Any()) return person.Name;
            return $"{person.Name} (+ {string.Join(", ", kids.Select(k => $"{k.Name} ×{k.Weight}"))})";
        }

        private string TourUrl(Tour tour)
            => $"{options.PublicBaseUrl.TrimEnd('/')}/goto/{tour.AccessCodeMD5}/{tour.Id}";

        /// <summary>
        /// Whether Telegram will accept this address on a button. It refuses anything it
        /// does not consider a real public address - a development server on localhost
        /// comes back as "Wrong HTTP URL" - and it refuses the whole message with it, so
        /// asking first is the difference between a reply and silence.
        /// </summary>
        internal static bool CanLinkTo(string url)
        {
            if (!Uri.TryCreate(url, UriKind.Absolute, out var uri)) return false;
            if (uri.Scheme != Uri.UriSchemeHttp && uri.Scheme != Uri.UriSchemeHttps) return false;
            if (uri.IsLoopback) return false;
            return uri.Host.Contains(".");
        }

        private TgReply Link(Tour tour)
        {
            var url = TourUrl(tour);
            if (CanLinkTo(url))
            {
                return TgReply.Say($"🧳 «{tour.Name}» в браузере — по кнопке. Код вводить не надо.")
                    .Row(TgButton.Link("Открыть Tourcalc", url));
            }
            // a development server: no button is possible, so the address goes in the text
            // where it can at least be copied
            return TgReply.Say($"🧳 «{tour.Name}»\n{url}");
        }

        /// <summary>The Mini App entry point for a tour - it proves who is looking before letting them in.</summary>
        private string MiniAppUrl(Tour tour)
            => $"{options.PublicBaseUrl.TrimEnd('/')}/tgapp?tour={tour.Id}";

        /// <summary>
        /// The "open it" button, when there is an address Telegram will take. In a private
        /// chat it opens as a Mini App - Telegram only allows those on an inline keyboard
        /// there - and in a group it is an ordinary link, which lands in the same in-app
        /// browser anyway.
        /// </summary>
        private void AddOpenButton(TgReply reply, Tour tour, bool isPrivate, params TgButton[] before)
        {
            var row = new List<TgButton>(before);
            if (CanLinkTo(TourUrl(tour)))
            {
                row.Add(isPrivate
                    ? TgButton.WebApp("Открыть", MiniAppUrl(tour))
                    : TgButton.Link("Открыть", TourUrl(tour)));
            }
            if (row.Count > 0) reply.Row(row.ToArray());
        }

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
