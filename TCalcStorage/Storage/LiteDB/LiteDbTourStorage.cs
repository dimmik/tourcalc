using System;
using System.Collections.Generic;
using System.Linq.Expressions;
using System.Text;
using TCalc.Domain;
using TCalc.Storage;

namespace TCalcStorage.Storage.LiteDB
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

        public IEnumerable<Tour> GetTours(Expression<Func<Tour, bool>> predicate, bool includeVersions, int from, int count, out int totalCount)
        {
            //return LiteDBTourStorageUtilities.LoadAllToursFromDb(dbPath, predicate, true, from, count);
            return LiteDBTourStorageUtilities.LoadAllToursFromDb(dbPath, predicate, includeVersions, from, count, out totalCount);
        }

        public IEnumerable<Tour> GetTourVersions(Expression<Func<Tour, bool>> predicate, string tourId, int from, int count, out int totalCount)
        {
            return LiteDBTourStorageUtilities.LoadTourVersionsFromDb(dbPath, predicate, tourId, from, count, out totalCount);
        }


        public void StoreTour(Tour tour)
        {
            tour.StoreToLiteDB(path: dbPath);
        }
    }
}
