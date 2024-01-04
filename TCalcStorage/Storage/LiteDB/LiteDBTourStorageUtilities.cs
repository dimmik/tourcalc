using Newtonsoft.Json;
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text;
using TCalc.Domain;
using LiteDB;
using System.Linq.Expressions;

namespace TCalcStorage.Storage.LiteDB
{
    public static class LiteDBTourStorageUtilities
    {

        public static IEnumerable<Tour> LoadAllToursFromDb(
            string path, 
            Expression<Func<Tour, bool>> predicate 
            , bool includeHistorical
            , int from, int count
            , out int totalCount)
        {
            if (predicate == null)
            {
                predicate = x => true;
            }
            using (var db = new LiteDatabase(path))
            {
                var q = db.GetCollection<Tour>("Tour").Find(predicate)
                    .Where(t => includeHistorical ? true : !t.IsVersion)
                    .OrderBy(t => t.DateCreated).Reverse();
                totalCount = q.Count();
                var tours = q
                    .Skip(from).Take(count);
                return tours.ToArray();
            }
        }

        public static IEnumerable<Tour> LoadTourVersionsFromDb(string path, Expression<Func<Tour, bool>> predicate
            , string tourId, int from, int count, out int totalCount)
        {
            if (predicate == null)
            {
                predicate = x => true;
            }
            using (var db = new LiteDatabase(path))
            {
                var q = db.GetCollection<Tour>("Tour").Find(predicate)
                    .Where(t => t.IsVersion && t.VersionFor_Id == tourId)
                    .OrderBy(t => t.DateVersioned).Reverse();
                totalCount = q.Count();
                var tours = q
                    .Skip(from).Take(count);
                return tours.ToArray();
            }
        }

        public static void DeleteTour(string path, string tourid)
        {
            using (var db = new LiteDatabase(path))
            {
                var col = db.GetCollection<Tour>("Tour");
                //col.Delete(x => x.GUID == tourid);
            }
        }


        // Using one .db file for each Tour (one collection per tour)
        public static void StoreToLiteDB (this Tour tour, string path)
        {
            UpsertTour(tour, path);
        }

        private static void UpsertTour(Tour tour, string path)
        {
            using (var db = new LiteDatabase(path))
            {
                var col = db.GetCollection<Tour>("Tour");
                if (col.Find(x => x.Id == tour.Id).Any())
                {
                    col.Update(tour);
                }
                else col.Insert(tour);
            }
        }

        public static void NewLiteDB (this Tour tour, string path)
        {
            tour.PrepareForStoring();
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
