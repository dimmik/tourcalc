using System;
using System.Collections.Generic;
using System.Linq.Expressions;
using System.Text;
using MongoDB.Driver;
using TCalc.Domain;
using System.Linq;
using System.Web;
using MongoDB.Bson.Serialization.Conventions;

namespace TCalc.Storage.MongoDB
{
    public class MongoDbTourStorage : ITourStorage
    {
        private readonly MongoClient client;

        public MongoDbTourStorage(string url, string username, string password)
        {
            var conventionPack = new ConventionPack { new IgnoreExtraElementsConvention(true) };
            ConventionRegistry.Register("IgnoreExtraElements", conventionPack, type => true);
            //            client = new MongoClient($"mongodb+srv://mongo:aAdmin001@dimmik-mongo-4su6a.azure.mongodb.net");
            client = new MongoClient($"mongodb+srv://{HttpUtility.UrlEncode(username)}:{HttpUtility.UrlEncode(password)}@{url}?connect=replicaSet");
            
            //check connectivity
            GetTour("none");
        }

        public void AddTour(Tour tour)
        {
            var db = client.GetDatabase("tour");
            var coll = db.GetCollection<Tour>("Tours");
            coll.InsertOne(tour);
        }

        public void DeleteTour(string tourid)
        {
            var db = client.GetDatabase("tour");
            var coll = db.GetCollection<Tour>("Tours");
            coll.DeleteOne(t => t.Id == tourid);
        }

        public Tour GetTour(string tourid)
        {
            var db = client.GetDatabase("tour");
            var coll = db.GetCollection<Tour>("Tours");
            Tour tour = coll.Find(t => t.Id == tourid).FirstOrDefault();
            return tour;
        }

        public IEnumerable<Tour> GetTours(Expression<Func<Tour, bool>> predicate, bool includeVersions, int from, int count, out int totalCount)
        {
            var db = client.GetDatabase("tour");
            var coll = db.GetCollection<Tour>("Tours");
            Expression<Func<Tour, bool>> isVersion;
            if (includeVersions)
            {
                isVersion = t => true;
            } else
            {
                isVersion = t => !t.IsVersion;
            }
            var allTours = coll
                .AsQueryable()
                .Where(isVersion)
                .Where(predicate)
                .OrderByDescending(t => t.DateCreated);
            totalCount = (int)allTours.Count();
            var tours = allTours.Skip(from).Take(count).ToList();
            return tours;
        }

        public IEnumerable<Tour> GetTourVersions(Expression<Func<Tour, bool>> predicate, string tourId, int from, int count, out int totalCount)
        {
            var db = client.GetDatabase("tour");
            var coll = db.GetCollection<Tour>("Tours");
            Expression<Func<Tour, bool>> vpr = (t) => (t.IsVersion && t.VersionFor_Id == tourId);

            var allVersions = coll.AsQueryable<Tour>()
                .Where(predicate)
                .Where(vpr)
                .OrderByDescending(t => t.DateVersioned);

            totalCount = (int)allVersions.Count();
            var versions = allVersions.Skip(from).Take(count).ToList();
            return versions;
        }

        public void StoreTour(Tour tour)
        {
            var db = client.GetDatabase("tour");
            var coll = db.GetCollection<Tour>("Tours");
            coll.ReplaceOne(t => t.Id == tour.Id, tour);
        }
    }
}
