using System;
using System.Collections.Generic;
using System.Linq.Expressions;
using System.Text;
using TCalc.Domain;
using TCalc.Storage.LiteDB;

namespace TCalc.Storage
{
    public class LiteDbTourStorage : ITourStorage
    {
        private readonly string dbPath;
        public LiteDbTourStorage(string path)
        {
            dbPath = path;
        }

        public void AddTour(Tour tour)
        {
            tour.NewLiteDB(dbPath);
        }

        public void DeleteTour(string tourid)
        {
            LiteDBTourStorageUtilities.DeleteTour(dbPath, tourid);
        }

        public Tour GetTour(string tourid)
        {
            return LiteDBTourStorageUtilities.LoadFromLiteDBbyId(dbPath, tourid);
        }

        public IEnumerable<Tour> GetTours(Expression<Func<Tour, bool>> predicate, bool includeVersions, int from, int count)
        {
            //return LiteDBTourStorageUtilities.LoadAllToursFromDb(dbPath, predicate, true, from, count);
            return LiteDBTourStorageUtilities.LoadAllToursFromDb(dbPath, predicate, includeVersions, from, count);
        }

        public IEnumerable<Tour> GetTourVersions(Expression<Func<Tour, bool>> predicate, string tourId, int from, int count)
        {
            return LiteDBTourStorageUtilities.LoadTourVersionsFromDb(dbPath, predicate, tourId, from, count);
        }

        public void StoreTour(Tour tour, bool version)
        {
            tour.StoreToLiteDB(path: dbPath, keepVersion: version);
        }
    }
}
