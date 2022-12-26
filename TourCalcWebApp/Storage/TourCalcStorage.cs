using Newtonsoft.Json;
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Linq.Expressions;
using System.Threading.Tasks;
using TCalc.Domain;
using TCalc.Storage;
using TCalc.Storage.MongoDB;
using TCalcStorage.Storage;
using TCalcStorage.Storage.LiteDB;
using TourCalcWebApp.Exceptions;

namespace TourCalcWebApp.Storage
{
    public class TourCalcStorage : ITourStorage
    {
        private readonly ITourStorage provider;
        private readonly ITcConfiguration Configuration;

        /*public TourCalcStorage(ITourStorage xprovider)
        {
            provider = xprovider;
        }*/
        public TourCalcStorage(ITcConfiguration config)
        {
            Configuration = config;
            //bool createVersions = Configuration.GetValue("TourVersioning", true);
            bool isVersionEditable = Configuration.GetValue("TourVersionEditable", false);

            var providerType = Configuration.GetValue("StorageType", "InMemory");
            if (providerType.ToLower() == "LiteDb".ToLower())
            {
                var rootFolder = Configuration.GetValue("DatabaseRootFolder", Path.DirectorySeparatorChar == '\\' ? @"d:\home\" : "/home/");
                var dbPath = $"{rootFolder}{Configuration.GetValue<string>("DatabaseRelativePath", $@"Tour.db")}";
                Directory.CreateDirectory(Path.GetDirectoryName(dbPath));
                provider = new LiteDbTourStorage(dbPath);
            }
            else if (providerType.ToLower() == "MongoDb".ToLower())
            {
                var url = Configuration.GetValue<string>("MongoDbUrl");
                var username = Configuration.GetValue<string>("MongoDbUsername");
                var password = Configuration.GetValue<string>("MongoDbPassword");
                provider = new MongoDbTourStorage(url, username, password);
            }
            else if (providerType.ToLower() == "InMemory".ToLower())
            {
                var fileName = Configuration.GetValue("InMemoryFileName", "inmemory-tours.json");
                List<Tour> tours = null;
                try {
                    var json = File.ReadAllText(fileName);
                    tours = JsonConvert.DeserializeObject<List<Tour>>(json);
                } catch
                {

                }
                
                provider = new InMemoryTourStorage(tours);
            }
            else
            {
                throw new ArgumentException($"Incorrect provider: {providerType}");
            }
        }

        public void AddTour(Tour tour)
        {
            tour.PrepareForStoring();
            provider.AddTour(tour);
        }

        public void DeleteTour(string tourid)
        {
            provider.DeleteTour(tourid);
        }

        public Tour GetTour(string tourid)
        {
            return provider.GetTour(tourid);
        }

        public IEnumerable<Tour> GetTours(Expression<Func<Tour, bool>> predicate, bool includeVersions, int from, int count, out int totalCount)
        {
            return provider.GetTours(predicate, includeVersions, from, count, out totalCount);
        }

        public IEnumerable<Tour> GetTourVersions(Expression<Func<Tour, bool>> predicate, string tourId, int from, int count, out int totalCount)
        {
            return provider.GetTourVersions(predicate, tourId, from, count, out totalCount);
        }

        public void StoreTour(Tour tour)
        {
            if (tour.IsVersion && !Configuration.GetValue("TourVersionEditable", false)) throw HttpException.Forbid("Versions are not editable");

            tour.PrepareForStoring();

            if (Configuration.GetValue("TourVersioning", true) && !tour.IsVersion) // do not version version
            {
                var tourVersion = GetTour(tour.Id);
                if (tourVersion != null)
                {
                    tourVersion.Id = Guid.NewGuid().ToString();
                    tourVersion.IsVersion = true;
                    tourVersion.DateVersioned = DateTime.Now;
                    tourVersion.VersionFor_Id = tour.Id;
                    bool doVersion = false;
                    (doVersion, tourVersion.VersionComment) = !string.IsNullOrWhiteSpace(tour.InternalVersionComment) ? (true, tour.InternalVersionComment) : new Func<(bool, string)>(() =>
                    {
                        if (tourVersion.Persons.Count() > tour.Persons.Count) return (true, $"P '{ tourVersion.Persons.Except(tour.Persons).LastOrDefault()?.Name ?? "--" }' deleted");
                        if (tourVersion.Persons.Count() < tour.Persons.Count) return (true, $"P '{ tour.Persons.Last()?.Name ?? "--" }' added");
                        var vSpendings = tourVersion.Spendings.Where(s => !s.Planned);
                        var tSpendings = tour.Spendings.Where(s => !s.Planned);
                        if (vSpendings.Count() < tSpendings.Count()) 
                            return (true, $"S '{tSpendings.LastOrDefault()?.Description ?? "--" } ({tSpendings.LastOrDefault()?.AmountInCents ?? 0} {tSpendings.LastOrDefault()?.Currency?.Name ?? "na"})' added");
                        if (vSpendings.Count() > tSpendings.Count()) 
                            return (true, $"S '{vSpendings.Except(tSpendings).LastOrDefault()?.Description ?? "--" } ({vSpendings.Except(tSpendings).LastOrDefault()?.AmountInCents ?? 0}  {vSpendings.LastOrDefault()?.Currency?.Name ?? "na"})' deleted");
                        if (tourVersion.IsArchived != tour.IsArchived)
                        {
                            if (tour.IsArchived) return (true, "Moved to archive");
                            if (!tour.IsArchived) return (true, "Restored from archive");
                        }
                        return GetChanges(tourVersion, tour);
                    })();
                    if (doVersion)
                    {
                        UpsertTour(tourVersion);
                    }
                }
            }
            UpsertTour(tour);
        }
        private static (bool, string) GetChanges(Tour oldTour, Tour newTour)
        {
            var res = "";
            bool doV = true;
            // persons
            res = PersonChanges(oldTour, newTour, res);
            // spendings
            res = SpendingChanges(oldTour, newTour, res);
            // tour attributes
            if (oldTour.Name != newTour.Name) res += $"Tour Name {oldTour.Name} -> {newTour.Name}";
            if (oldTour.IsFinalizing != newTour.IsFinalizing) res += $"Tour Finalizing flag: {oldTour.IsFinalizing} -> {newTour.IsFinalizing}";
            if (string.IsNullOrWhiteSpace(res))
            {
                res = "-";
                doV = false;
            }
            return (doV, $"Changed: {res}");
        }

        private static string SpendingChanges(Tour oldTour, Tour newTour, string res)
        {
            var zippedS = oldTour.Spendings.Where(s => !s.Planned).OrderBy(p => p.GUID)
                .Zip(newTour.Spendings.Where(s => !s.Planned).OrderBy(p => p.GUID));
            var zippedSChanged = zippedS.Where(
                fs => fs.First.Description != fs.Second.Description
                    || fs.First.AmountInCents != fs.Second.AmountInCents
                    || fs.First.ToAll != fs.Second.ToAll
                    || fs.First.FromGuid != fs.Second.FromGuid
                    || fs.First.ToGuid.Count != fs.Second.ToGuid.Count
                    || fs.First.Type != fs.Second.Type
                    || fs.First.Currency != fs.Second.Currency
                );
            foreach (var (olds, news) in zippedSChanged)
            {
                if (olds.Description != news.Description) res += $"Spending: {olds.Description} -> {news.Description}; ";
                if (olds.AmountInCents != news.AmountInCents) res += $"{olds.Description}: {olds.AmountInCents} -> {news.AmountInCents}; ";
                if (olds.ToAll != news.ToAll) res += $"{olds.Description} toAll: {olds.ToAll} -> {news.ToAll}; ";
                if (olds.FromGuid != news.FromGuid) res += $"{olds.Description} From: {oldTour.Persons.FirstOrDefault(p => p.GUID == olds.FromGuid)?.Name ?? "na"}"
                        + $" -> {newTour.Persons.FirstOrDefault(p => p.GUID == news.FromGuid)?.Name ?? "na"}; ";
                if (olds.ToGuid.Count != news.ToGuid.Count) res += $"{olds.Description} To Count: {olds.ToGuid.Count} -> {news.ToGuid.Count}; ";
                if (olds.Type != news.Type) res += $"{olds.Description} Type: {olds.Type} -> {news.Type}; ";
                if (olds.Currency != news.Currency) res += $" {olds.Description} ({olds.AmountInCents} {olds.Currency.Name}) Currency: {olds.Currency.Name} -> {news.Currency.Name}";
            }

            return res;
        }

        private static string PersonChanges(Tour oldTour, Tour newTour, string res)
        {
            var zippedP = oldTour.Persons.OrderBy(p => p.GUID).Zip(newTour.Persons.OrderBy(p => p.GUID));
            var zippedPChanged = zippedP
                .Where((fs) => fs.First.Name != fs.Second.Name || fs.First.Weight != fs.Second.Weight || fs.First.ParentId != fs.Second.ParentId);
            foreach (var (oldp, newp) in zippedPChanged) // due to bulk update might some, not only one
            {
                if (oldp.Name != newp.Name) res += $"Person: {oldp.Name} -> {newp.Name}; ";
                if (oldp.Weight != newp.Weight) res += $"{oldp.Name} Weight: {oldp.Weight} -> {newp.Weight}; ";
                if (oldp.ParentId != newp.ParentId)
                    res += $"{oldp.Name} Parent: {oldTour.Persons.FirstOrDefault(p => p.GUID == oldp.ParentId)?.Name ?? "None"} -> "
                        + $"{newTour.Persons.FirstOrDefault(p => p.GUID == newp.ParentId)?.Name ?? "None"}; ";
            }

            return res;
        }

        private void UpsertTour(Tour tour)
        {
            var sameTour = provider.GetTour(tour.Id);
            if (sameTour == null)
            {
                provider.AddTour(tour);
            } else
            {
                provider.StoreTour(tour);
            }
        }
    }
}
