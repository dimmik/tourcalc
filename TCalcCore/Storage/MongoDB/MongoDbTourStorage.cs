using System;
using System.Collections.Generic;
using System.Linq.Expressions;
using System.Text;
using MongoDB.Driver;
using TCalc.Domain;

namespace TCalc.Storage.MongoDB
{
    public class MongoDbTourStorage : ITourStorage
    {
        private readonly MongoClient client;

        public MongoDbTourStorage()
        {
            client = new MongoClient("mongodb+srv://mongo:aAdmin001@dimmik-mongo-4su6a.azure.mongodb.net?retryWrites=true&w=majority");
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
            var allTours = coll.Find(predicate).SortByDescending(t => t.DateCreated);
            totalCount = (int)allTours.CountDocuments();
            var tours = allTours.Skip(from).Limit(count).ToList();
            return tours;
        }

        public IEnumerable<Tour> GetTourVersions(Expression<Func<Tour, bool>> predicate, string tourId, int from, int count, out int totalCount)
        {
            var db = client.GetDatabase("tour");
            var coll = db.GetCollection<Tour>("Tours");
            Expression<Func<Tour, bool>> vpr = (t) => (t.IsVersion && t.VersionFor_Id == tourId);
            predicate = predicate.Update(vpr, new[] { Expression.Parameter(typeof(Boolean)) });

            var allVersions = coll.Find(predicate)
                    .SortByDescending(t => t.DateVersioned);
            totalCount = (int)allVersions.CountDocuments();
            var versions = allVersions.Skip(from).Limit(count).ToList();
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
