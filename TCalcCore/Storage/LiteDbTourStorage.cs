using System;
using System.Collections.Generic;
using System.Linq.Expressions;
using System.Text;
using TCalc.Domain;

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
            TourStorageUtilities.DeleteTour(dbPath, tourid);
        }

        public Tour GetTour(string tourid)
        {
            return TourStorageUtilities.LoadFromLiteDBbyId(dbPath, tourid);
        }

        public IEnumerable<Tour> GetTours(Expression<Func<Tour, bool>> predicate = null)
        {
            return TourStorageUtilities.LoadAllToursFromDb(dbPath, predicate);
        }

        public void StoreStour(Tour tour)
        {
            tour.StoreToLiteDB(dbPath);
        }
    }
}
