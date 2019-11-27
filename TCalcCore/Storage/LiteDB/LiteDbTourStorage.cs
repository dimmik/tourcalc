using System;
using System.Collections.Generic;
using System.Linq.Expressions;
using System.Text;
using TCalc.Domain;

namespace TCalc.Storage.LiteDB
{
    public class LiteDbTourStorage : ITourStorage
    {
        private readonly bool CreateVersions = false;
        private readonly bool IsVersionEditable = false;
        private readonly string dbPath;
        public LiteDbTourStorage(string path, bool createVersions, bool isVersionEditable)
        {
            dbPath = path;
            CreateVersions = createVersions;
            IsVersionEditable = isVersionEditable;
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
            if (tour.IsVersion && !IsVersionEditable) throw new TourStorageException("Versions are not editable");
            tour.StoreToLiteDB(path: dbPath, keepVersion: CreateVersions);
        }
    }
    public class TourStorageException : Exception
    {
        public TourStorageException(string msg) : base(msg) { }
    }
}
