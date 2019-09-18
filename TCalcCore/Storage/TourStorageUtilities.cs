using Newtonsoft.Json;
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text;
using TCalc.Domain;
using LiteDB;
using System.Linq.Expressions;

namespace TCalc.Storage
{
    public static class TourStorageUtilities
    {

        public static IEnumerable<Tour> LoadAllToursFromDb(string path, Expression<Func<Tour, bool>> predicate = null)
        {
            if (predicate == null)
            {
                predicate = x => true;
            }
            using (var db = new LiteDatabase(path))
            {
                return db.GetCollection<Tour>("Tour").Find(predicate);
            }
        }
        public static void DeleteTour(string path, string tourid)
        {
            using (var db = new LiteDatabase(path))
            {
                var col = db.GetCollection<Tour>("Tour");
                col.Delete(x => x.GUID == tourid);
            }
        }
        // Using one .db file for single Tour
        public static void StoreToLiteDB (this Tour tour, string path)
        {
            using (var db = new LiteDatabase(path))
            {
                var col = db.GetCollection<Tour>("Tour");
                if (col.Find(x => x.GUID == tour.GUID) != null)
                {
                    col.Update(tour);
                }
                else col.Insert(tour);
            }
        }

        public static void NewLiteDB (this Tour tour, string path)
        {
            using (var db = new LiteDatabase(path))
            {
                var col = db.GetCollection<Tour>("Tour");
                col.Insert(tour);
            }
        }

        // using one .db for all tours
        public static Tour LoadFromLiteDBbyId(string path, string id)
        {
            Tour tour = null;
            using (var db = new LiteDatabase(path))
            {
                tour = db.GetCollection<Tour>("Tour").FindOne(x => x.GUID == id);
            }
            return tour;
        }
    }
}
