using System;
using System.Collections.Generic;
using System.Linq;
using System.Text.RegularExpressions;
using System.Threading.Tasks;
using TCalc.Domain;
using TCalc.Logic;
using TCalc.Storage;
using Telegram.Bot.Types;
using TourCalcWebApp.Auth;

namespace TourCalcWebApp.TgBot
{
    public class TourBotService
    {
        readonly private ITourStorage TourStorage;
        readonly private Chat Chat;
        readonly private User FromUser;

        public TourBotService(ITourStorage tourStorage, Chat chat, User user)
        {
            TourStorage = tourStorage;
            Chat = chat;
            FromUser = user;
        }

        public string Perform(string command, string rest)
        {
            // handle commands like /show@tourcalc_bot
            if (command.Contains("@"))
            {
                command = command.Substring(0, command.IndexOf('@'));
            }
            switch (command)
            {
                case "/rename": 
                    return Rename(rest);
                case "/new":
                    return AddNew(rest);
                case "/addme":
                    return AddMe(rest);
                case "/spend":
                    return Spend(rest);
                case "/weblink":
                    return WebLink();
                case "/people":
                    return People();
                case "/summary": // users and debt
                    return Summary();
                case "/debt": // my own debt
                    return Debt();
                case "/help": 
                    return Help();
                default:
                    return UnknownCommand(command);
            }
        }

        private string Help()
        {
            return @"/new - initialize tour for the group
/rename {newname} - rename a tour
/addme {displayname, optional} - add yourself to a tour
/spend {description with amount} - add spending
/people - list people in the tour
/summary - people with debt
/debt - who you should pay to
/weblink - link to the tour in web
/help - this page
";
        }

        private Tour GetOrCreateTour(string name = "")
        {
            string tourid = GenerateId(Chat.Id);
            Tour tour = TourStorage.GetTour(tourid);
            if (tour == null)
            {
                var guid = $"{Guid.NewGuid()}";
                tour = new Tour()
                {
                    Id = tourid,
                    Name = string.IsNullOrWhiteSpace(name) ? $"Telegram Tour for {Chat.Title}" : name,
                    Metadata = guid,
                    AccessCodeMD5 = guid.CreateMD5()
                };
                TourStorage.AddTour(tour);
            }
            return tour;
        }

        private string GenerateId(long id)
        {
            return $"tg-{id}";
        }

        private string UnknownCommand(string command)
        {
            return $"Unknown command {command}";
        }

        private string Debt()
        {
            var tour = GetOrCreateTour();
            if (!tour.Persons.Any(p => p.GUID == $"{FromUser.Id}"))
            {
                return $"{FromUser.FirstName} {FromUser.LastName}, you are not in tour";
            }
            var calculator = new TourCalculator(tour);
            var calculated = calculator.Calculate(includePlanned: true);
            var suggestions = calculated.Spendings
                .Where(s => s.Planned)
                .Where(s => s.FromGuid == $"{FromUser.Id}")
                .Select(s =>
                    $"from {tour.Persons.Where(p => p.GUID == s.FromGuid).First().Name} to {tour.Persons.Where(p => p.GUID == s.ToGuid.First()).First().Name} pay {s.AmountInCents}"
                    );
            return $"Tour '{calculated.Name}' payments from {FromUser.FirstName} {FromUser.LastName}\r\n" +
                $"{(suggestions.Any() ?  string.Join("\r\n", suggestions) : $"Nothing to pay, {FromUser.FirstName} {FromUser.LastName}")}";
        }

        private string Summary()
        {
            var tour = GetOrCreateTour();
            var calculator = new TourCalculator(tour);
            var calculated = calculator.Calculate(includePlanned: false);
            return $"Tour '{calculated.Name}\r\n" +
                $"{string.Join("\r\n", calculated.Persons.Select(p => $"{p.Name} ({p.Weight}) debt: {p.ReceivedInCents - p.SpentInCents}"))}";
        }

        private string People()
        {
            var tour = GetOrCreateTour();
            var msg = string.Join("\r\n", tour.Persons.Select(p => $"{p.Name}"));
            return $"Tour '{tour.Name}' people ({tour.Persons.Count()})\r\n{msg}";
        }

        private string WebLink()
        {
            var tour = GetOrCreateTour();
            var link = $"https://tourcalc.azurewebsites.net/goto/{tour.Metadata}/{tour.Id}";
            return link;
        }

        private string Spend(string content)
        {
            AddMe("");
            var tour = GetOrCreateTour();
            (int amount, string comment, bool ok) = GetAmountFromString(content);
            if (ok)
            {
                var spending = new Spending()
                {
                    GUID = $"{Guid.NewGuid()}",
                    FromGuid = $"{FromUser.Id}",
                    AmountInCents = amount,
                    ToAll = true,
                    Description = comment
                };
                tour.Spendings.Add(spending);
                TourStorage.StoreTour(tour);
                return $"Added spending from '{tour.Persons.Where(p => p.GUID == $"{FromUser.Id}").First().Name}' to all: {amount} ({comment})";
            } else
            {
                return $"Cant understand how many has been spent in '{content}'";
            }
        }
        // TODO
        private (int amount, string comment, bool ok) GetAmountFromString(string content)
        {
            content = content.Trim();
            // TODO calculate
            string pattern;
            try
            {
                // starts with number
                pattern = @"^([0-9]+)\s+(.+)$";
                Regex r = new Regex(pattern);
                if (r.IsMatch(content))
                {
                    var match = r.Match(content);
                    var amount = int.Parse(match.Groups[1].Value);
                    var desc = match.Groups[2].Value;
                    return (amount, desc, true);
                }
                // ends with number
                pattern = @"^(.+)\s+([0-9]+)$";
                r = new Regex(pattern);
                if (r.IsMatch(content))
                {
                    var match = r.Match(content);
                    var desc = match.Groups[1].Value;
                    var amount = int.Parse(match.Groups[2].Value);
                    return (amount, desc, true);
                }
                // the only number in string
                pattern = @"^[^0-9]+\s+([0-9]+)\s+[^0-9]+$";
                r = new Regex(pattern);
                if (r.IsMatch(content))
                {
                    var match = r.Match(content);
                    var desc = match.Groups[0].Value;
                    var amount = int.Parse(match.Groups[1].Value);
                    return (amount, desc, true);
                }
                return (0, content, false);
            } catch
            {
                return (0, content, false);
            }
        }

        private string AddMe(string name)
        {
            var tour = GetOrCreateTour();
            var personId = $"{FromUser.Id}";
            var username =
                string.IsNullOrWhiteSpace(FromUser.LastName) && string.IsNullOrWhiteSpace(FromUser.FirstName)
                ? $"({FromUser.Id})" + name
                : $"{FromUser.FirstName} {FromUser.LastName}";
            if (tour.Persons.Any(p => p.GUID == personId))
            {
                var person = tour.Persons.Where(p => p.GUID == personId).First();
                if (!string.IsNullOrWhiteSpace(name))
                {
                    person.Name = $"Tg {name}";
                    TourStorage.StoreTour(tour);
                }
                return $"{username}, You are already in the tour. Name now '{person.Name}'";
            }
            var pp = new Person()
            {
                GUID = $"{FromUser.Id}",
                Name = $"Tg {username}"
            };
            if (!string.IsNullOrWhiteSpace(name))
            {
                pp.Name = $"Tg {name}";
            }
            tour.Persons.Add(pp);
            TourStorage.StoreTour(tour);
            return $"{username}, I added you ({pp.Name})";
        }

        private string AddNew(string name)
        {
            var tour = GetOrCreateTour(name);
            return $"Tour is here: {tour.Name}";
        }

        private string Rename(string name)
        {
            var tour = GetOrCreateTour(name);
            if (!string.IsNullOrWhiteSpace(name))
            {
                tour.Name = name;
            }
            TourStorage.StoreTour(tour);
            return $"Tour renamed: {tour.Name}";
        }
    }
}
