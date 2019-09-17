using Newtonsoft.Json;
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text;
using TCalc.Domain;
using LiteDB;

namespace TCalc.Logic
{
    public static class TourStorageUtilities
    {
        public static Tour LoadTourFromJson(this string json)
        {
            var tour = Deserialize<Tour>(json);
            return tour;
        }
        public static string AsJson(this Tour tour)
        {
            var json = Serialize(tour);
            return json;
        }

        public static IEnumerable<Tour> LoadAllTours(string path)
        {
            using (var db = new LiteDatabase(path))
            {
                return db.GetCollection<Tour>("Tour").FindAll();
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

        public static Tour LoadFromLiteDB (string path)
        {
            Tour tour = null;
            using (var db = new LiteDatabase(path))
            {
                tour = db.GetCollection<Tour>("Tour").FindOne(x => x != null);
            }
            return tour;
        }

        public static Tour LoadTourFromFolder(string path)
        {
            var files = Directory.EnumerateFiles(path, "*.json").OrderBy(fn => fn);
            Tour tour = new Tour();
            var rwlock = getRwLock(path);
            lock (rwlock) {
                // load tour data
                if (files.Any(f => Path.GetFileName(f).Equals("Tour.json", StringComparison.InvariantCultureIgnoreCase)))
                {
                    var json = File.ReadAllText(
                        files.FirstOrDefault(f => Path.GetFileName(f).Equals("Tour.json", StringComparison.InvariantCultureIgnoreCase)));
                    tour = Deserialize<Tour>(json);
                    tour.Persons = new List<Person>();
                    tour.Spendings = new List<Spending>();
                }
                else
                {
                    throw new Exception($"Cannot find 'Tour.json' in '{path}'");
                }
                // load persons
                foreach (var p in files.Where(f => Path.GetFileName(f).ToLower().StartsWith("person")))
                {
                    var json = File.ReadAllText(p);
                    var person = Deserialize<Person>(json);
                    tour.Persons.Add(person);
                }
                // load spendings
                foreach (var s in files.Where(f => Path.GetFileName(f).ToLower().StartsWith("spending")))
                {
                    var json = File.ReadAllText(s);
                    var spending = Deserialize<Spending>(json);
                    tour.Spendings.Add(spending);
                }
            }
            return tour;
        }
        public static void StoreToFolder(this Tour tour, string path)
        {
            string json;
            // store tour
            json = CleanAndSerializeTour(tour);
            var rwlock = getRwLock(path);
            lock (rwlock)
            {

                File.WriteAllText($@"{path}\Tour.json", json);
                // store persons
                int i = 1;
                foreach (var p in tour.Persons)
                {
                    if (!p.IsFromJson) p.Order = i;
                    i++;
                    if (p.IsChanged)
                    {
                        var fn = $@"{path}\Person_{p.FileId()}.json";
                        json = Serialize(p);
                        File.WriteAllText(fn, json);
                    }
                }
                // store spendings
                i = 1;
                foreach (var s in tour.Spendings)
                {
                    if (!s.IsFromJson) s.Order = i;
                    i++;
                    if (s.IsChanged)
                    {
                        var fn = $@"{path}\Spending_{s.FileId()}.json";
                        json = Serialize(s);
                        File.WriteAllText(fn, json);
                    }
                }
            }
        }

        private static string CleanAndSerializeTour(Tour tour)
        {
            var pp = tour.Persons;
            var ss = tour.Spendings;
            tour.Persons = null;
            tour.Spendings = null;
            string json = Serialize(tour);
            tour.Persons = pp;
            tour.Spendings = ss;
            return json;
        }

        private static string Serialize(object o)
        {
            return JsonConvert.SerializeObject(o, Formatting.Indented);
        }
        private static T Deserialize<T>(string json) where T : AbstractItem
        {
            T res = JsonConvert.DeserializeObject<T>(json);
            //res.IsChanged = false; // have not found the way to save only changed yet. For later
            res.IsChanged = true;
            res.IsFromJson = true;
            return res;
        }
        private static readonly Dictionary<string, object> rwlockD = new Dictionary<string, object>();
        private static readonly object lck = new object();
        private static object getRwLock(string path)
        {
            var absP = Path.GetFullPath(path);
            lock (lck)
            {
                if (rwlockD.ContainsKey(absP))
                {
                    return rwlockD[absP];
                }
                else
                {
                    var l = new object();
                    rwlockD[absP] = l;
                    return l;
                }
            }
        }
    }
}
