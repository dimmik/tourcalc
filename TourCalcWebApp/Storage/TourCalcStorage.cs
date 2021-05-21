using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Linq.Expressions;
using System.Threading.Tasks;
using TCalc.Domain;
using TCalc.Storage;
using TCalc.Storage.MongoDB;
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

            var providerType = Configuration.GetValue("StorageType", "LiteDb");
            if (providerType.ToLower() == "LiteDb".ToLower())
            {
                var rootFolder = Configuration.GetValue("DatabaseRootFolder", Path.DirectorySeparatorChar == '\\' ? @"d:\home\" : "/home/");
                var dbPath = $"{rootFolder}{Configuration.GetValue<string>("DatabaseRelativePath", $@"Tour.db")}";
                Directory.CreateDirectory(Path.GetDirectoryName(dbPath));
                provider = new LiteDbTourStorage(dbPath);
            } else if (providerType.ToLower() == "MongoDb".ToLower())
            {
                var url = Configuration.GetValue<string>("MongoDbUrl");
                var username = Configuration.GetValue<string>("MongoDbUsername");
                var password = Configuration.GetValue<string>("MongoDbPassword");
                provider = new MongoDbTourStorage(url, username, password);
            } else
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
                    tourVersion.VersionComment = tour.InternalVersionComment ?? new Func<string>(() =>
                    {
                        if (tourVersion.Persons.Count() > tour.Persons.Count) return $"P '{ tourVersion.Persons.Except(tour.Persons).Last()?.Name ?? "--" }' deleted";
                        if (tourVersion.Persons.Count() < tour.Persons.Count) return $"P '{ tour.Persons.Last()?.Name ?? "--" }' added";
                        var vSpendings = tourVersion.Spendings.Where(s => !s.Planned);
                        var tSpendings = tour.Spendings.Where(s => !s.Planned);
                        if (vSpendings.Count() < tSpendings.Count()) return $"S '{tSpendings.Last()?.Description ?? "--" }' added";
                        if (vSpendings.Count() > tSpendings.Count()) return $"S '{vSpendings.Except(tSpendings).Last()?.Description ?? "--" }' deleted";
                        return "Names or numbers changed";
                    })();
                    UpsertTour(tourVersion);
                }
            }
            UpsertTour(tour);
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
