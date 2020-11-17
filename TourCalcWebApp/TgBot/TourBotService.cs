using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using TCalc.Domain;
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

        public string Perform(string command, params string[] args)
        {
            switch (command)
            {
                case "/rename": 
                    return Rename(args);
                case "/new":
                    return AddNew(args);
                case "/addme":
                    return AddMe(args);
                case "/spend":
                    return Spend(args);
                case "/weblink":
                    return WebLink(args);
                case "/people":
                    return People(args);
                case "/summary": // users and debt
                    return Summary(args);
                case "/debt": // my own debt
                    return Debt(args);
                default:
                    return UnknownCommand(command, args);
            }
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

        private string UnknownCommand(string command, string[] args)
        {
            return $"Unknown command {command}";
        }

        private string Debt(string[] args)
        {
            return "[TODO] debt";
        }

        private string Summary(string[] args)
        {
            return "TODO summary";
        }

        private string People(string[] args)
        {
            var tour = GetOrCreateTour();
            var msg = string.Join("\r\n", tour.Persons.Select(p => $"{p.Name}"));
            return $"Tour {tour.Name}\r\n{msg}";
        }

        private string WebLink(string[] args)
        {
            var tour = GetOrCreateTour();
            var link = $"https://tourcalc.azurewebsites.net/goto/{tour.Metadata}/{tour.Id}";
            return link;
        }

        private string Spend(string[] args)
        {
            return "TODO spend";
        }

        private string AddMe(string[] args)
        {
            var tour = GetOrCreateTour();
            var personId = $"{FromUser.Id}";
            var name = string.Join(" ", args);
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
                }
                TourStorage.StoreTour(tour);
                return $"{username}, You are already in the tour. Name {person.Name}";
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

        private string AddNew(string[] args)
        {
            var str = string.Join(" ", args);
            var tour = GetOrCreateTour(str);
            return $"Tour is here: {tour.Name}";
        }

        private string Rename(string[] args)
        {
            var str = string.Join(" ", args);
            var tour = GetOrCreateTour(str);
            if (!string.IsNullOrWhiteSpace(str))
            {
                tour.Name = str;
            }
            TourStorage.StoreTour(tour);
            return $"Tour renamed: {tour.Name}";
        }
    }
}
